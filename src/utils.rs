use std::ffi::CString;

use crate::bindings::{TAG_NULL, TAG_OBJECT};
use crate::ValueError;

use libquickjs_sys as q;

/// Free a JSValue.
/// This function is the equivalent of JS_FreeValue from quickjs, which can not
/// be used due to being `static inline`.
pub unsafe fn free_value(context: *mut q::JSContext, value: q::JSValue) {
    // All tags < 0 are garbage collected and need to be freed.
    if value.tag < 0 {
        // This transmute is OK since if tag < 0, the union will be a refcount
        // pointer.
        let ptr = value.u.ptr as *mut q::JSRefCountHeader;
        let pref: &mut q::JSRefCountHeader = &mut *ptr;
        pref.ref_count -= 1;
        if pref.ref_count <= 0 {
            q::__JS_FreeValue(context, value);
        }
    }
}

#[cfg(feature = "chrono")]
pub fn js_date_constructor(context: *mut q::JSContext) -> q::JSValue {
    let global = unsafe { q::JS_GetGlobalObject(context) };
    assert_eq!(global.tag, TAG_OBJECT);

    let date_constructor = unsafe {
        q::JS_GetPropertyStr(
            context,
            global,
            std::ffi::CStr::from_bytes_with_nul(b"Date\0")
                .unwrap()
                .as_ptr(),
        )
    };
    assert_eq!(date_constructor.tag, TAG_OBJECT);
    unsafe { free_value(context, global) };
    date_constructor
}

#[cfg(feature = "bigint")]
pub fn js_create_bigint_function(context: *mut q::JSContext) -> q::JSValue {
    let global = unsafe { q::JS_GetGlobalObject(context) };
    assert_eq!(global.tag, TAG_OBJECT);

    let bigint_function = unsafe {
        q::JS_GetPropertyStr(
            context,
            global,
            std::ffi::CStr::from_bytes_with_nul(b"BigInt\0")
                .unwrap()
                .as_ptr(),
        )
    };
    assert_eq!(bigint_function.tag, TAG_OBJECT);
    unsafe { free_value(context, global) };
    bigint_function
}

/// Helper for creating CStrings.
pub fn make_cstring(value: impl Into<Vec<u8>>) -> Result<CString, ValueError> {
    CString::new(value).map_err(ValueError::StringWithZeroBytes)
}

/// Helper to construct null JsValue
pub fn js_null_value() -> q::JSValue {
    q::JSValue {
        u: q::JSValueUnion { int32: 0 },
        tag: TAG_NULL,
    }
}
