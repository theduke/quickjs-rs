use std::ffi::CStr;
use std::str::Utf8Error;

use libquickjs_sys::{
    JSContext, JSValue, JS_FreeCString, JS_FreeValue, JS_GetException, JS_IsException, JS_IsNull,
    JS_IsString, JS_ToCStringLen2, JS_ToString,
};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum Internal {
    #[error("Unexpected null pointer")]
    UnexpectedNullPointer,
    #[error("Unexpected null value")]
    UnexpectedNullValue,
    #[error("Expected string")]
    ExpectedString,
    #[error("Invalid UTF-8")]
    InvalidUtf8(#[from] Utf8Error),
    #[error("Nul byte found in string")]
    NulError(#[from] std::ffi::NulError),
}

unsafe fn get_string(context: *mut JSContext, value: JSValue) -> Result<String, Internal> {
    if !JS_IsString(value) {
        return Err(Internal::ExpectedString);
    }

    // convert to a rust string
    let ptr = JS_ToCStringLen2(context, std::ptr::null_mut(), value, 0);

    if ptr.is_null() {
        return Err(Internal::UnexpectedNullPointer);
    }

    let c_str = CStr::from_ptr(ptr);

    let string = c_str.to_str()?.to_string();

    // Free the C string
    JS_FreeCString(context, ptr);

    Ok(string)
}

unsafe fn exception_to_string(
    context: *mut JSContext,
    exception: JSValue,
) -> Result<String, Internal> {
    if JS_IsNull(exception) {
        return Err(Internal::UnexpectedNullValue);
    }

    let exception = if JS_IsString(exception) {
        exception
    } else {
        JS_ToString(context, exception)
    };

    get_string(context, exception)
}

#[derive(Debug, Clone, Error)]
pub enum SerializationError {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Internal error: {0}")]
    Internal(#[from] Internal),
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("Expected call to `serialize_key` before `serialize_value`")]
    MissingKey,
    #[error("Expected call times of calls to `serialize_key` and `serialize_value` to be equal")]
    MissingValue,
    #[error("Expected either a string or a number as a key")]
    InvalidKey,
    #[error("The serializer is in an invalid state")]
    InvalidState,
    #[error("The number is too large to be represented")]
    IntTooLarge,
}

impl SerializationError {
    pub fn from_exception(context: *mut JSContext) -> Self {
        // https://bellard.org/quickjs/quickjs.html#Exceptions 3.4.4
        let exception = unsafe { JS_GetException(context) };

        let value = unsafe { exception_to_string(context, exception) };

        match value {
            Ok(value) => {
                if value.contains("out of memory") {
                    Self::OutOfMemory
                } else {
                    Self::Unknown(value)
                }
            }
            Err(err) => err.into(),
        }
    }

    pub fn try_from_value(context: *mut JSContext, value: JSValue) -> Result<JSValue, Self> {
        if unsafe { JS_IsException(value) } {
            // we're for sure an error, we just don't know which one
            // TODO: do we need to free here?
            unsafe { JS_FreeValue(context, value) }

            Err(Self::from_exception(context))
        } else {
            Ok(value)
        }
    }
}

impl serde::ser::Error for SerializationError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self::Unknown(msg.to_string())
    }
}
