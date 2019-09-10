use std::{collections::HashMap, error, fmt};

/// A value that can be (de)serialized to/from the quickjs runtime.
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
    /// Only available with the optional `num-bigint` feature
    #[cfg(feature = "num-bigint")]
    BigInt(crate::BigInt),
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

#[cfg(feature = "num-bigint")]
impl From<i64> for JsValue {
    fn from(value: i64) -> Self {
        JsValue::BigInt(value.into())
    }
}

#[cfg(feature = "num-bigint")]
impl From<num_bigint::BigInt> for JsValue {
    fn from(value: num_bigint::BigInt) -> Self {
        JsValue::BigInt(value.into())
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
            StringWithZeroBytes(_) => write!(f, "String contains \\0 bytes",),
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

    #[cfg(feature = "num-bigint")]
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

    #[cfg(feature = "num-bigint")]
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

    #[cfg(feature = "num-bigint")]
    #[test]
    fn test_bigint_i64_bigint_eq() {
        let value_i64 = JsValue::BigInt(1234i64.into());
        let value_bigint = JsValue::BigInt(num_bigint::BigInt::from(1234i64).into());
        assert_eq!(value_i64, value_bigint);
    }
}
