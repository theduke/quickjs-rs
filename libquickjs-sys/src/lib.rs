//! FFI Bindings for [quickjs](https://bellard.org/quickjs/),
//! a Javascript engine.
//! See the [quickjs](https://crates.io/crates/quickjs) crate for a high-level
//! wrapper.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// import the functions from static-functions.c

extern "C" {
    fn JS_DupValue_real(ctx: *mut JSContext, v: JSValue);
    fn JS_FreeValue_real(ctx: *mut JSContext, v: JSValue);
    fn JS_NewBool_real(ctx: *mut JSContext, v: bool) -> JSValue;
    fn JS_NewInt32_real(ctx: *mut JSContext, v: i32) -> JSValue;
    fn JS_NewFloat64_real(ctx: *mut JSContext, v: f64) -> JSValue;
}

pub fn JS_DupValue(ctx: *mut JSContext, v: JSValue) {
    unsafe {JS_DupValue_real(ctx, v)};
}

pub fn JS_FreeValue(ctx: *mut JSContext, v: JSValue) {
    unsafe {JS_FreeValue_real(ctx, v)};
}

pub fn JS_NewBool(ctx: *mut JSContext, v: bool) -> JSValue {
    unsafe {JS_NewBool_real(ctx, v)}
}

pub fn JS_NewInt32(ctx: *mut JSContext, v: i32) -> JSValue {
    unsafe {JS_NewInt32_real(ctx, v)}
}

pub fn JS_NewFloat64(ctx: *mut JSContext, v: f64) -> JSValue {
    unsafe {JS_NewFloat64_real(ctx, v)}
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::*;

    // Small sanity test that starts the runtime and evaluates code.
    #[test]
    fn test_eval() {
        unsafe {



            let rt = JS_NewRuntime();
            let ctx = JS_NewContext(rt);

            let code_str = "1 + 1\0";
            let code = CStr::from_bytes_with_nul(code_str.as_bytes()).unwrap();
            let script = CStr::from_bytes_with_nul("script\0".as_bytes()).unwrap();

            let value = JS_Eval(
                ctx,
                code.as_ptr(),
                (code_str.len() - 1) as u64,
                script.as_ptr(),
                JS_EVAL_TYPE_GLOBAL as i32,
            );
            assert_eq!(value.tag, 0);
            assert_eq!(value.u.int32, 2);

            JS_DupValue(ctx, value);
            JS_FreeValue(ctx, value);

            let ival = JS_NewInt32(ctx, 12);
            assert_eq!(ival.tag, 0);
            let fval = JS_NewFloat64(ctx, f64::MAX);
            assert_eq!(fval.tag, 7);
            let bval = JS_NewBool(ctx, true);
            assert_eq!(bval.tag, 1);

        }
    }
}
