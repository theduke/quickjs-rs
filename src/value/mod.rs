#[cfg(feature = "bigint")]
pub(crate) mod bigint;
pub(crate) mod marshal;

use std::{collections::HashMap, error, fmt};

use crate::owned_value_ref::OwnedValueRef;
#[cfg(feature = "bigint")]
pub use bigint::BigInt;
use crate::ExecutionError;
use crate::utils::js_null_value;
use crate::marshal::{serialize_value, deserialize_value};

/// A value that can be (de)serialized to/from the quickjs runtime.
/// See the marshal module.
#[derive(PartialEq, Clone, Debug)]
#[allow(missing_docs)]
pub enum JsValue {
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(HashMap<String, JsValue>),
    /// chrono::Datetime<Utc> / JS Date integration.
    /// Only available with the optional `chrono` feature.
    #[cfg(feature = "chrono")]
    Date(chrono::DateTime<chrono::Utc>),
    /// num_bigint::BigInt / JS BigInt integration
    /// Only available with the optional `bigint` feature
    #[cfg(feature = "bigint")]
    BigInt(crate::BigInt),
    OpaqueFunction(OwnedValueRef),
    #[doc(hidden)]
    __NonExhaustive,
}

impl JsValue {
    /// Cast value to a str.
    ///
    /// Returns `Some(&str)` if value is a `JsValue::String`, None otherwise.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsValue::String(ref s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Convert to `String`.
    pub fn into_string(self) -> Option<String> {
        match self {
            JsValue::String(s) => Some(s),
            _ => None,
        }
    }
}

macro_rules! value_impl_from {
    (
        (
            $(  $t1:ty => $var1:ident, )*
        )
        (
            $( $t2:ty => |$exprname:ident| $expr:expr => $var2:ident, )*
        )
    ) => {
        $(
            impl From<$t1> for JsValue {
                fn from(value: $t1) -> Self {
                    JsValue::$var1(value)
                }
            }

            impl std::convert::TryFrom<JsValue> for $t1 {
                type Error = ValueError;

                fn try_from(value: JsValue) -> Result<Self, Self::Error> {
                    match value {
                        JsValue::$var1(inner) => Ok(inner),
                        _ => Err(ValueError::UnexpectedType)
                    }

                }
            }
        )*
        $(
            impl From<$t2> for JsValue {
                fn from(value: $t2) -> Self {
                    let $exprname = value;
                    let inner = $expr;
                    JsValue::$var2(inner)
                }
            }
        )*
    }
}

value_impl_from! {
    (
        bool => Bool,
        i32 => Int,
        f64 => Float,
        String => String,
    )
    (
        i8 => |x| i32::from(x) => Int,
        i16 => |x| i32::from(x) => Int,
        u8 => |x| i32::from(x) => Int,
        u16 => |x| i32::from(x) => Int,
        u32 => |x| f64::from(x) => Float,
    )
}

#[cfg(feature = "bigint")]
value_impl_from! {
    ()
    (
        i64 => |x| x.into() => BigInt,
        u64 => |x| num_bigint::BigInt::from(x).into() => BigInt,
        i128 => |x| num_bigint::BigInt::from(x).into() => BigInt,
        u128 => |x| num_bigint::BigInt::from(x).into() => BigInt,
        num_bigint::BigInt => |x| x.into() => BigInt,
    )
}

#[cfg(feature = "bigint")]
impl std::convert::TryFrom<JsValue> for i64 {
    type Error = ValueError;

    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        match value {
            JsValue::Int(int) => Ok(int as i64),
            JsValue::BigInt(bigint) => bigint.as_i64().ok_or(ValueError::UnexpectedType),
            _ => Err(ValueError::UnexpectedType),
        }
    }
}

#[cfg(feature = "bigint")]
macro_rules! value_bigint_impl_tryfrom {
    (
        ($($t:ty => $to_type:ident, )*)
    ) => {
        $(
            impl std::convert::TryFrom<JsValue> for $t {
                type Error = ValueError;

                fn try_from(value: JsValue) -> Result<Self, Self::Error> {
                    use num_traits::ToPrimitive;

                    match value {
                        JsValue::Int(int) => Ok(int as $t),
                        JsValue::BigInt(bigint) => bigint
                            .into_bigint()
                            .$to_type()
                            .ok_or(ValueError::UnexpectedType),
                        _ => Err(ValueError::UnexpectedType),
                    }
                }
            }
        )*
    }
}

#[cfg(feature = "bigint")]
value_bigint_impl_tryfrom! {
    (
        u64 => to_u64,
        i128 => to_i128,
        u128 => to_u128,
    )
}

#[cfg(feature = "bigint")]
impl std::convert::TryFrom<JsValue> for num_bigint::BigInt {
    type Error = ValueError;

    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        match value {
            JsValue::Int(int) => Ok(num_bigint::BigInt::from(int)),
            JsValue::BigInt(bigint) => Ok(bigint.into_bigint()),
            _ => Err(ValueError::UnexpectedType),
        }
    }
}

impl<T> From<Vec<T>> for JsValue
    where
        T: Into<JsValue>,
{
    fn from(values: Vec<T>) -> Self {
        let items = values.into_iter().map(|x| x.into()).collect();
        JsValue::Array(items)
    }
}

impl<'a> From<&'a str> for JsValue {
    fn from(val: &'a str) -> Self {
        JsValue::String(val.into())
    }
}

impl<T> From<Option<T>> for JsValue
    where
        T: Into<JsValue>,
{
    fn from(opt: Option<T>) -> Self {
        if let Some(value) = opt {
            value.into()
        } else {
            JsValue::Null
        }
    }
}

impl<K, V> From<HashMap<K, V>> for JsValue
    where
        K: Into<String>,
        V: Into<JsValue>,
{
    fn from(map: HashMap<K, V>) -> Self {
        let new_map = map.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        JsValue::Object(new_map)
    }
}

/// Represents a JS function. Can be used for calling back into JS code.
///
/// # Example
/// ```rust
/// use quick_js::OpaqueJsFunction;
/// fn register_state_listener(id: String, callback_function: OpaqueJsFunction) -> String {
///    callback_function.invoke(vec![id]);
///    id
///}
///
/// fn main() -> Result<(), dyn std::error::Error> {
/// use quick_js::Context;
/// let mut context = Context::builder().build()?;
/// context.add_callback("notifyOnThingStatesChange", register_state_listener)?;
/// Ok(())
/// }
/// ```
pub struct OpaqueJsFunction(OwnedValueRef);

impl OpaqueJsFunction {
    /// Calls the js function
    pub fn invoke(
        &self,
        args: impl IntoIterator<Item = impl Into<JsValue>>,
    ) -> Result<JsValue, ExecutionError> {
        let mut qargs :Vec<q::JSValue>= args
            .into_iter()
            .map(|arg| serialize_value(self.0.context,arg.into()).unwrap_or(js_null_value()))
            .collect();

        use libquickjs_sys as q;

        let qres_raw = unsafe {
            q::JS_Call(
                self.0.context,
                self.0.value,
                js_null_value(),
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        let qres = OwnedValueRef::wrap(self.0.context, qres_raw);
        Ok(deserialize_value(self.0.context, &qres.value)?)
    }
}

impl std::convert::TryFrom<JsValue> for OpaqueJsFunction {
    type Error = ValueError;

    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        match value {
            JsValue::OpaqueFunction(value) => Ok(OpaqueJsFunction(value)),
            _ => Err(ValueError::UnexpectedType),
        }
    }
}

/// Can be used as generic argument in a callback function.
/// Can only contain basic types, no functions, no objects, no nested ones.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum JsSimpleArgumentValue {
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    String(String),
    /// chrono::Datetime<Utc> / JS Date integration.
    /// Only available with the optional `chrono` feature.
    #[cfg(feature = "chrono")]
    Date(chrono::DateTime<chrono::Utc>),
    /// num_bigint::BigInt / JS BigInt integration
    /// Only available with the optional `bigint` feature
    #[cfg(feature = "bigint")]
    BigInt(BigInt),
}

impl std::convert::TryFrom<JsValue> for JsSimpleArgumentValue {
    type Error = ValueError;

    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        match value {
            JsValue::Null => Ok(JsSimpleArgumentValue::Null),
            JsValue::Bool(value) => Ok(JsSimpleArgumentValue::Bool(value)),
            JsValue::Int(value) => Ok(JsSimpleArgumentValue::Int(value)),
            JsValue::Float(value) => Ok(JsSimpleArgumentValue::Float(value)),
            JsValue::String(value) => Ok(JsSimpleArgumentValue::String(value)),
            #[cfg(feature = "chrono")]
            JsValue::Date(value) => Ok(JsSimpleArgumentValue::Date(value)),
            #[cfg(feature = "bigint")]
            JsValue::BigInt(value) => Ok(JsSimpleArgumentValue::BigInt(value)),
            _ => Err(ValueError::UnexpectedType),
        }
    }
}

/// Error during value conversion.
#[derive(PartialEq, Eq, Debug)]
pub enum ValueError {
    /// Invalid non-utf8 string.
    InvalidString(std::str::Utf8Error),
    /// Encountered string with \0 bytes.
    StringWithZeroBytes(std::ffi::NulError),
    /// Internal error.
    Internal(String),
    /// Received an unexpected type that could not be converted.
    UnexpectedType,
    #[doc(hidden)]
    __NonExhaustive,
}

// TODO: remove this once either the Never type get's stabilized or the compiler
// can properly handle Infallible.
impl From<std::convert::Infallible> for ValueError {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!()
    }
}

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ValueError::*;
        match self {
            InvalidString(e) => write!(
                f,
                "Value conversion failed - invalid non-utf8 string: {}",
                e
            ),
            StringWithZeroBytes(_) => write!(f, "String contains \\0 bytes", ),
            Internal(e) => write!(f, "Value conversion failed - internal error: {}", e),
            UnexpectedType => write!(f, "Could not convert - received unexpected type"),
            __NonExhaustive => unreachable!(),
        }
    }
}

impl error::Error for ValueError {}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(feature = "bigint")]
    #[test]
    fn test_bigint_from_i64() {
        let int = 1234i64;
        let value = JsValue::from(int);
        if let JsValue::BigInt(value) = value {
            assert_eq!(value.as_i64(), Some(int));
        } else {
            panic!("Expected JsValue::BigInt");
        }
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn test_bigint_from_bigint() {
        let bigint = num_bigint::BigInt::from(std::i128::MAX);
        let value = JsValue::from(bigint.clone());
        if let JsValue::BigInt(value) = value {
            assert_eq!(value.into_bigint(), bigint);
        } else {
            panic!("Expected JsValue::BigInt");
        }
    }

    #[cfg(feature = "bigint")]
    #[test]
    fn test_bigint_i64_bigint_eq() {
        let value_i64 = JsValue::BigInt(1234i64.into());
        let value_bigint = JsValue::BigInt(num_bigint::BigInt::from(1234i64).into());
        assert_eq!(value_i64, value_bigint);
    }
}
