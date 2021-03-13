use libquickjs_sys as q;

use crate::{ExecutionError, JsValue, ValueError};

use super::make_cstring;
use crate::bindings::ContextWrapper;

#[repr(i32)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum JsTag {
    // Used by C code as a marker.
    // Not relevant for bindings.
    // First = q::JS_TAG_FIRST,
    Int = q::JS_TAG_INT,
    Bool = q::JS_TAG_BOOL,
    Null = q::JS_TAG_NULL,
    Module = q::JS_TAG_MODULE,
    Object = q::JS_TAG_OBJECT,
    String = q::JS_TAG_STRING,
    Symbol = q::JS_TAG_SYMBOL,
    #[cfg(feature = "bigint")]
    BigInt = q::JS_TAG_BIG_INT,
    Float64 = q::JS_TAG_FLOAT64,
    BigFloat = q::JS_TAG_BIG_FLOAT,
    Exception = q::JS_TAG_EXCEPTION,
    Undefined = q::JS_TAG_UNDEFINED,
    BigDecimal = q::JS_TAG_BIG_DECIMAL,
    CatchOffset = q::JS_TAG_CATCH_OFFSET,
    Uninitialized = q::JS_TAG_UNINITIALIZED,
    FunctionBytecode = q::JS_TAG_FUNCTION_BYTECODE,
}

impl JsTag {
    #[inline]
    pub(super) fn from_c(value: &q::JSValue) -> JsTag {
        let inner = unsafe { q::JS_ValueGetTag(*value) };
        match inner {
            q::JS_TAG_INT => JsTag::Int,
            q::JS_TAG_BOOL => JsTag::Bool,
            q::JS_TAG_NULL => JsTag::Null,
            q::JS_TAG_MODULE => JsTag::Module,
            q::JS_TAG_OBJECT => JsTag::Object,
            q::JS_TAG_STRING => JsTag::String,
            q::JS_TAG_SYMBOL => JsTag::Symbol,
            q::JS_TAG_FLOAT64 => JsTag::Float64,
            q::JS_TAG_BIG_FLOAT => JsTag::BigFloat,
            q::JS_TAG_EXCEPTION => JsTag::Exception,
            q::JS_TAG_UNDEFINED => JsTag::Undefined,
            q::JS_TAG_BIG_DECIMAL => JsTag::BigDecimal,
            q::JS_TAG_CATCH_OFFSET => JsTag::CatchOffset,
            q::JS_TAG_UNINITIALIZED => JsTag::Uninitialized,
            q::JS_TAG_FUNCTION_BYTECODE => JsTag::FunctionBytecode,
            #[cfg(feature = "bigint")]
            q::JS_TAG_BIG_INT => JsTag::BigInt,
            _other => {
                unreachable!()
            }
        }
    }

    pub(super) fn to_c(self) -> i32 {
        // TODO: figure out why this is needed
        // Just casting with `as` does not work correctly
        match self {
            JsTag::Int => q::JS_TAG_INT,
            JsTag::Bool => q::JS_TAG_BOOL,
            JsTag::Null => q::JS_TAG_NULL,
            JsTag::Module => q::JS_TAG_MODULE,
            JsTag::Object => q::JS_TAG_OBJECT,
            JsTag::String => q::JS_TAG_STRING,
            JsTag::Symbol => q::JS_TAG_SYMBOL,
            JsTag::Float64 => q::JS_TAG_FLOAT64,
            JsTag::BigFloat => q::JS_TAG_BIG_FLOAT,
            JsTag::Exception => q::JS_TAG_EXCEPTION,
            JsTag::Undefined => q::JS_TAG_UNDEFINED,
            JsTag::BigDecimal => q::JS_TAG_BIG_DECIMAL,
            JsTag::CatchOffset => q::JS_TAG_CATCH_OFFSET,
            JsTag::Uninitialized => q::JS_TAG_UNINITIALIZED,
            JsTag::FunctionBytecode => q::JS_TAG_FUNCTION_BYTECODE,
            #[cfg(feature = "bigint")]
            JsTag::BigInt => q::JS_TAG_FUNCTION_BYTECODE,
        }
    }

    /// Returns `true` if the js_tag is [`Undefined`].
    #[inline]
    pub fn is_undefined(&self) -> bool {
        matches!(self, Self::Undefined)
    }

    /// Returns `true` if the js_tag is [`Object`].
    #[inline]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object)
    }

    /// Returns `true` if the js_tag is [`Exception`].
    #[inline]
    pub fn is_exception(&self) -> bool {
        matches!(self, Self::Exception)
    }

    /// Returns `true` if the js_tag is [`Int`].
    #[inline]
    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int)
    }

    /// Returns `true` if the js_tag is [`Bool`].
    #[inline]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool)
    }

    /// Returns `true` if the js_tag is [`Null`].
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns `true` if the js_tag is [`Module`].
    #[inline]
    pub fn is_module(&self) -> bool {
        matches!(self, Self::Module)
    }

    /// Returns `true` if the js_tag is [`String`].
    #[inline]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String)
    }

    /// Returns `true` if the js_tag is [`Symbol`].
    #[inline]
    pub fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol)
    }

    /// Returns `true` if the js_tag is [`BigInt`].
    #[cfg(feature = "bigint")]
    #[inline]
    pub fn is_big_int(&self) -> bool {
        matches!(self, Self::BigInt)
    }

    /// Returns `true` if the js_tag is [`Float64`].
    #[inline]
    pub fn is_float64(&self) -> bool {
        matches!(self, Self::Float64)
    }

    /// Returns `true` if the js_tag is [`BigFloat`].
    #[inline]
    pub fn is_big_float(&self) -> bool {
        matches!(self, Self::BigFloat)
    }

    /// Returns `true` if the js_tag is [`BigDecimal`].
    #[inline]
    pub fn is_big_decimal(&self) -> bool {
        matches!(self, Self::BigDecimal)
    }
}

pub struct OwnedJsAtom<'a> {
    context: &'a ContextWrapper,
    value: q::JSAtom,
}

impl<'a> OwnedJsAtom<'a> {
    #[inline]
    pub fn new(context: &'a ContextWrapper, value: q::JSAtom) -> Self {
        Self { context, value }
    }
}

impl<'a> Drop for OwnedJsAtom<'a> {
    fn drop(&mut self) {
        unsafe {
            q::JS_FreeAtom(self.context.context, self.value);
        }
    }
}

impl<'a> Clone for OwnedJsAtom<'a> {
    fn clone(&self) -> Self {
        unsafe { q::JS_DupAtom(self.context.context, self.value) };
        Self {
            context: self.context,
            value: self.value,
        }
    }
}

/// OwnedJsValue wraps a Javascript value owned by the QuickJs runtime.
///
/// Guarantees cleanup of resources by dropping the value from the runtime.
///
/// ### Comparison to [`crate::JsValue`]:
///
/// `JsValue` is a native Rust value that can be converted to QuickJs native
/// types. `OwnedJsValue`, in contrast, owns the underlying QuickJs runtime
/// value directly.
// TODO: provide usage docs.
pub struct OwnedJsValue<'a> {
    context: &'a ContextWrapper,
    // FIXME: make private again, just for testing
    pub(crate) value: q::JSValue,
}

impl<'a> OwnedJsValue<'a> {
    #[inline]
    pub(crate) fn context(&self) -> &ContextWrapper {
        self.context
    }

    #[inline]
    pub(crate) fn new(context: &'a ContextWrapper, value: q::JSValue) -> Self {
        Self { context, value }
    }

    #[inline]
    pub(crate) fn tag(&self) -> JsTag {
        JsTag::from_c(&self.value)
    }

    /// Get the inner JSValue without increasing ref count.
    ///
    /// Unsafe because the caller must ensure proper memory management.
    pub(super) unsafe fn as_inner(&self) -> &q::JSValue {
        &self.value
    }

    /// Extract the underlying JSValue.
    ///
    /// Unsafe because the caller must ensure memory management. (eg JS_FreeValue)
    pub(super) unsafe fn extract(self) -> q::JSValue {
        let v = self.value;
        std::mem::forget(self);
        v
    }

    /// Check if this value is `null`.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.tag().is_null()
    }

    /// Check if this value is `undefined`.
    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.tag() == JsTag::Undefined
    }

    /// Check if this value is `bool`.
    #[inline]
    pub fn is_bool(&self) -> bool {
        self.tag() == JsTag::Bool
    }

    /// Check if this value is a Javascript exception.
    #[inline]
    pub fn is_exception(&self) -> bool {
        self.tag() == JsTag::Exception
    }

    /// Check if this value is a Javascript object.
    #[inline]
    pub fn is_object(&self) -> bool {
        self.tag() == JsTag::Object
    }

    /// Check if this value is a Javascript array.
    #[inline]
    pub fn is_array(&self) -> bool {
        unsafe { q::JS_IsArray(self.context.context, self.value) == 1 }
    }

    /// Check if this value is a Javascript function.
    #[inline]
    pub fn is_function(&self) -> bool {
        unsafe { q::JS_IsFunction(self.context.context, self.value) == 1 }
    }

    /// Check if this value is a Javascript module.
    #[inline]
    pub fn is_module(&self) -> bool {
        self.tag().is_module()
    }

    /// Check if this value is a Javascript string.
    #[inline]
    pub fn is_string(&self) -> bool {
        self.tag() == JsTag::String
    }

    /// Check if this value is a bytecode compiled function.
    #[inline]
    pub fn is_compiled_function(&self) -> bool {
        self.tag() == JsTag::FunctionBytecode
    }

    /// Serialize this value into a [`JsValue`].
    pub fn to_value(&self) -> Result<JsValue, ValueError> {
        self.context.to_value(&self.value)
    }

    pub(crate) fn to_bool(&self) -> Result<bool, ValueError> {
        match self.to_value()? {
            JsValue::Bool(b) => Ok(b),
            _ => Err(ValueError::UnexpectedType),
        }
    }

    pub(crate) fn try_into_object(self) -> Result<OwnedJsObject<'a>, ValueError> {
        OwnedJsObject::try_from_value(self)
    }

    pub(crate) fn try_into_function(self) -> Result<JsFunction<'a>, ValueError> {
        JsFunction::try_from_value(self)
    }

    pub(crate) fn try_into_compiled_function(self) -> Result<JsCompiledFunction<'a>, ValueError> {
        JsCompiledFunction::try_from_value(self)
    }

    pub(crate) fn try_into_module(self) -> Result<JsModule<'a>, ValueError> {
        JsModule::try_from_value(self)
    }

    /// Call the Javascript `.toString()` method on this value.
    pub(crate) fn js_to_string(&self) -> Result<String, ExecutionError> {
        let value = if self.is_string() {
            self.to_value()?
        } else {
            let raw = unsafe { q::JS_ToString(self.context.context, self.value) };
            let value = OwnedJsValue::new(self.context, raw);

            if !value.is_string() {
                return Err(ExecutionError::Exception(
                    "Could not convert value to string".into(),
                ));
            }
            value.to_value()?
        };

        Ok(value.as_str().unwrap().to_string())
    }

    #[cfg(test)]
    pub(crate) fn get_ref_count(&self) -> i32 {
        if self.value.tag < 0 {
            // This transmute is OK since if tag < 0, the union will be a refcount
            // pointer.
            let ptr = unsafe { self.value.u.ptr as *mut q::JSRefCountHeader };
            let pref: &mut q::JSRefCountHeader = &mut unsafe { *ptr };
            pref.ref_count
        } else {
            -1
        }
    }
}

impl<'a> Drop for OwnedJsValue<'a> {
    fn drop(&mut self) {
        unsafe {
            q::JS_FreeValue(self.context.context, self.value);
        }
    }
}

impl<'a> Clone for OwnedJsValue<'a> {
    fn clone(&self) -> Self {
        unsafe { q::JS_DupValue(self.context.context, self.value) };
        Self {
            context: self.context,
            value: self.value,
        }
    }
}

impl<'a> std::fmt::Debug for OwnedJsValue<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}(_)", self.tag())
    }
}

pub struct OwnedJsArray<'a> {
    value: OwnedJsValue<'a>,
}

impl<'a> OwnedJsArray<'a> {
    pub fn new(value: OwnedJsValue<'a>) -> Option<Self> {
        if value.is_array() {
            Some(Self { value })
        } else {
            None
        }
    }
}

/// Wraps an object from the QuickJs runtime.
/// Provides convenience property accessors.
#[derive(Clone, Debug)]
pub struct OwnedJsObject<'a> {
    value: OwnedJsValue<'a>,
}

impl<'a> OwnedJsObject<'a> {
    pub fn try_from_value(value: OwnedJsValue<'a>) -> Result<Self, ValueError> {
        if !value.is_object() {
            Err(ValueError::Internal("Expected an object".into()))
        } else {
            Ok(Self { value })
        }
    }

    pub fn into_value(self) -> OwnedJsValue<'a> {
        self.value
    }

    pub fn property(&self, name: &str) -> Result<Option<OwnedJsValue<'a>>, ExecutionError> {
        // TODO: prevent allocation
        let cname = make_cstring(name)?;
        let value = {
            let raw = unsafe {
                q::JS_GetPropertyStr(self.value.context.context, self.value.value, cname.as_ptr())
            };
            OwnedJsValue::new(self.value.context, raw)
        };
        let tag = value.tag();

        if tag.is_exception() {
            Err(ExecutionError::Internal(format!(
                "Exception while getting property '{}'",
                name
            )))
        } else if tag.is_undefined() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    pub fn property_require(&self, name: &str) -> Result<OwnedJsValue<'a>, ExecutionError> {
        self.property(name)?
            .ok_or_else(|| ExecutionError::Internal(format!("Property '{}' not found", name)))
    }

    /// Determine if the object is a promise by checking the presence of
    /// a 'then' and a 'catch' property.
    pub fn is_promise(&self) -> Result<bool, ExecutionError> {
        if let Some(p) = self.property("then")? {
            if p.is_function() {
                return Ok(true);
            }
        }
        if let Some(p) = self.property("catch")? {
            if p.is_function() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn set_property(&self, name: &str, value: OwnedJsValue<'a>) -> Result<(), ExecutionError> {
        let cname = make_cstring(name)?;
        unsafe {
            // NOTE: SetPropertyStr takes ownership of the value.
            // We do not, however, call OwnedJsValue::extract immediately, so
            // the inner JSValue is still managed.
            // `mem::forget` is called below only if SetProperty succeeds.
            // This prevents leaks when an error occurs.
            let ret = q::JS_SetPropertyStr(
                self.value.context.context,
                self.value.value,
                cname.as_ptr(),
                value.value,
            );

            if ret < 0 {
                Err(ExecutionError::Exception("Could not set property".into()))
            } else {
                // Now we can call forget to prevent calling the destructor.
                std::mem::forget(value);
                Ok(())
            }
        }
    }
}

/// Wraps an object from the QuickJs runtime.
/// Provides convenience property accessors.
#[derive(Clone, Debug)]
pub struct JsFunction<'a> {
    value: OwnedJsValue<'a>,
}

impl<'a> JsFunction<'a> {
    pub fn try_from_value(value: OwnedJsValue<'a>) -> Result<Self, ValueError> {
        if !value.is_function() {
            Err(ValueError::Internal(format!(
                "Expected a function, got {:?}",
                value.tag()
            )))
        } else {
            Ok(Self { value })
        }
    }

    pub fn into_value(self) -> OwnedJsValue<'a> {
        self.value
    }

    pub fn call(&self, args: Vec<OwnedJsValue<'a>>) -> Result<OwnedJsValue<'a>, ExecutionError> {
        let mut qargs = args.iter().map(|arg| arg.value).collect::<Vec<_>>();

        let qres_raw = unsafe {
            q::JS_Call(
                self.value.context.context,
                self.value.value,
                q::JSValue {
                    u: q::JSValueUnion { int32: 0 },
                    tag: JsTag::Null as i64,
                },
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        Ok(OwnedJsValue::new(self.value.context, qres_raw))
    }
}

/// A bytecode compiled function.
#[derive(Clone, Debug)]
pub struct JsCompiledFunction<'a> {
    value: OwnedJsValue<'a>,
}

impl<'a> JsCompiledFunction<'a> {
    pub(crate) fn try_from_value(value: OwnedJsValue<'a>) -> Result<Self, ValueError> {
        if !value.is_compiled_function() {
            Err(ValueError::Internal(format!(
                "Expected a compiled function, got {:?}",
                value.tag()
            )))
        } else {
            Ok(Self { value })
        }
    }

    pub(crate) fn as_value(&self) -> &OwnedJsValue<'_> {
        &self.value
    }

    pub(crate) fn into_value(self) -> OwnedJsValue<'a> {
        self.value
    }

    /// Evaluate this compiled function and return the resulting value.
    // FIXME: add example
    pub fn eval(&'a self) -> Result<OwnedJsValue<'a>, ExecutionError> {
        super::compile::run_compiled_function(self)
    }

    /// Convert this compiled function into QuickJS bytecode.
    ///
    /// Bytecode can be stored and loaded with [`Context::compile`].
    // FIXME: add example
    pub fn to_bytecode(&self) -> Result<Vec<u8>, ExecutionError> {
        Ok(super::compile::to_bytecode(self.value.context, self))
    }
}

/// A bytecode compiled module.
pub struct JsModule<'a> {
    value: OwnedJsValue<'a>,
}

impl<'a> JsModule<'a> {
    pub fn try_from_value(value: OwnedJsValue<'a>) -> Result<Self, ValueError> {
        if !value.is_module() {
            Err(ValueError::Internal(format!(
                "Expected a compiled function, got {:?}",
                value.tag()
            )))
        } else {
            Ok(Self { value })
        }
    }

    pub fn into_value(self) -> OwnedJsValue<'a> {
        self.value
    }
}

/// The result of loading QuickJs bytecode.
/// Either a function or a module.
pub enum JsCompiledValue<'a> {
    Function(JsCompiledFunction<'a>),
    Module(JsModule<'a>),
}
