mod compile;
mod convert;
mod droppable_value;
mod value;

use std::{
    ffi::CString,
    os::raw::{c_int, c_void},
    sync::Mutex,
};

use libquickjs_sys as q;

use crate::{
    callback::{Arguments, Callback},
    console::ConsoleBackend,
    ContextError, ExecutionError, JsValue, ValueError,
};

#[cfg(feature = "bigint")]
use crate::value::{bigint::BigIntOrI64, BigInt};

use value::{JsFunction, OwnedJsObject};

pub use value::{JsCompiledFunction, OwnedJsValue};

// JS_TAG_* constants from quickjs.
// For some reason bindgen does not pick them up.
#[cfg(feature = "bigint")]
const TAG_BIG_INT: i64 = -10;
const TAG_STRING: i64 = -7;
const TAG_FUNCTION_BYTECODE: i64 = -2;
const TAG_OBJECT: i64 = -1;
const TAG_INT: i64 = 0;
const TAG_BOOL: i64 = 1;
const TAG_NULL: i64 = 2;
const TAG_UNDEFINED: i64 = 3;
const TAG_EXCEPTION: i64 = 6;
const TAG_FLOAT64: i64 = 7;

/// Helper for creating CStrings.
fn make_cstring(value: impl Into<Vec<u8>>) -> Result<CString, ValueError> {
    CString::new(value).map_err(ValueError::StringWithZeroBytes)
}

type WrappedCallback = dyn Fn(c_int, *mut q::JSValue) -> q::JSValue;

/// Taken from: https://s3.amazonaws.com/temp.michaelfbryan.com/callbacks/index.html
///
/// Create a C wrapper function for a Rust closure to enable using it as a
/// callback function in the Quickjs runtime.
///
/// Both the boxed closure and the boxed data are returned and must be stored
/// by the caller to guarantee they stay alive.
unsafe fn build_closure_trampoline<F>(
    closure: F,
) -> ((Box<WrappedCallback>, Box<q::JSValue>), q::JSCFunctionData)
where
    F: Fn(c_int, *mut q::JSValue) -> q::JSValue + 'static,
{
    unsafe extern "C" fn trampoline<F>(
        _ctx: *mut q::JSContext,
        _this: q::JSValue,
        argc: c_int,
        argv: *mut q::JSValue,
        _magic: c_int,
        data: *mut q::JSValue,
    ) -> q::JSValue
    where
        F: Fn(c_int, *mut q::JSValue) -> q::JSValue,
    {
        let closure_ptr = (*data).u.ptr;
        let closure: &mut F = &mut *(closure_ptr as *mut F);
        (*closure)(argc, argv)
    }

    let boxed_f = Box::new(closure);

    let data = Box::new(q::JSValue {
        u: q::JSValueUnion {
            ptr: (&*boxed_f) as *const F as *mut c_void,
        },
        tag: TAG_NULL,
    });

    ((boxed_f, data), Some(trampoline::<F>))
}

/// OwnedValueRef wraps a Javascript value from the quickjs runtime.
/// It prevents leaks by ensuring that the inner value is deallocated on drop.
pub struct OwnedValueRef<'a> {
    context: &'a ContextWrapper,
    value: q::JSValue,
}

impl<'a> Drop for OwnedValueRef<'a> {
    fn drop(&mut self) {
        unsafe {
            q::JS_FreeValue(self.context.context, self.value);
        }
    }
}

impl<'a> Clone for OwnedValueRef<'a> {
    fn clone(&self) -> Self {
        Self::new_dup(self.context, self.value)
    }
}

impl<'a> std::fmt::Debug for OwnedValueRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.value.tag {
            TAG_EXCEPTION => write!(f, "Exception(?)"),
            TAG_NULL => write!(f, "NULL"),
            TAG_UNDEFINED => write!(f, "UNDEFINED"),
            TAG_BOOL => write!(f, "Bool(?)",),
            TAG_INT => write!(f, "Int(?)"),
            TAG_FLOAT64 => write!(f, "Float(?)"),
            TAG_STRING => write!(f, "String(?)"),
            TAG_OBJECT => write!(f, "Object(?)"),
            TAG_FUNCTION_BYTECODE => write!(f, "Bytecode(?)"),
            _ => write!(f, "?"),
        }
    }
}

impl<'a> OwnedValueRef<'a> {
    pub fn new(context: &'a ContextWrapper, value: q::JSValue) -> Self {
        Self { context, value }
    }
    pub fn new_dup(context: &'a ContextWrapper, value: q::JSValue) -> Self {
        let ret = Self::new(context, value);
        unsafe { q::JS_DupValue(ret.context.context, ret.value) };
        ret
    }

    /// Get the inner JSValue without freeing in drop.
    ///
    /// Unsafe because the caller is responsible for freeing the returned value.
    unsafe fn into_inner(self) -> q::JSValue {
        let v = self.value;
        std::mem::forget(self);
        v
    }

    /// Get the inner JSValue without increasing ref count
    pub(crate) fn as_inner(&self) -> &q::JSValue {
        &self.value
    }

    /// Get the inner JSValue while increasing ref count, this is handy when you pass a JSValue to a new owner like e.g. setProperty
    #[allow(dead_code)]
    pub(crate) fn as_inner_dup(&self) -> &q::JSValue {
        unsafe { q::JS_DupValue(self.context.context, self.value) };
        &self.value
    }

    pub fn is_null(&self) -> bool {
        self.value.tag == TAG_NULL
    }

    pub fn is_bool(&self) -> bool {
        self.value.tag == TAG_BOOL
    }

    pub fn is_exception(&self) -> bool {
        self.value.tag == TAG_EXCEPTION
    }

    pub fn is_object(&self) -> bool {
        self.value.tag == TAG_OBJECT
    }

    pub fn is_string(&self) -> bool {
        self.value.tag == TAG_STRING
    }

    pub fn is_compiled_function(&self) -> bool {
        self.value.tag == TAG_FUNCTION_BYTECODE
    }

    pub fn to_string(&self) -> Result<String, ExecutionError> {
        let value = if self.is_string() {
            self.to_value()?
        } else {
            let raw = unsafe { q::JS_ToString(self.context.context, self.value) };
            let value = OwnedValueRef::new(self.context, raw);

            if value.value.tag != TAG_STRING {
                return Err(ExecutionError::Exception(
                    "Could not convert value to string".into(),
                ));
            }
            value.to_value()?
        };

        Ok(value.as_str().unwrap().to_string())
    }

    pub fn to_value(&self) -> Result<JsValue, ValueError> {
        self.context.to_value(&self.value)
    }

    pub fn to_bool(&self) -> Result<bool, ValueError> {
        match self.to_value()? {
            JsValue::Bool(b) => Ok(b),
            _ => Err(ValueError::UnexpectedType),
        }
    }

    #[cfg(test)]
    pub fn get_ref_count(&self) -> i32 {
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

/// Wraps an object from the quickjs runtime.
/// Provides convenience property accessors.
pub struct OwnedObjectRef<'a> {
    value: OwnedValueRef<'a>,
}

impl<'a> OwnedObjectRef<'a> {
    pub fn new(value: OwnedValueRef<'a>) -> Result<Self, ValueError> {
        if value.value.tag != TAG_OBJECT {
            Err(ValueError::Internal("Expected an object".into()))
        } else {
            Ok(Self { value })
        }
    }

    fn into_value(self) -> OwnedValueRef<'a> {
        self.value
    }

    /// Get the tag of a property.
    fn property_tag(&self, name: &str) -> Result<i64, ValueError> {
        let cname = make_cstring(name)?;
        let raw = unsafe {
            q::JS_GetPropertyStr(self.value.context.context, self.value.value, cname.as_ptr())
        };
        let t = raw.tag;
        unsafe {
            q::JS_FreeValue(self.value.context.context, raw);
        }
        Ok(t)
    }

    /// Determine if the object is a promise by checking the presence of
    /// a 'then' and a 'catch' property.
    fn is_promise(&self) -> Result<bool, ValueError> {
        if self.property_tag("then")? == TAG_OBJECT && self.property_tag("catch")? == TAG_OBJECT {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn property(&self, name: &str) -> Result<OwnedValueRef<'a>, ExecutionError> {
        let cname = make_cstring(name)?;
        let raw = unsafe {
            q::JS_GetPropertyStr(self.value.context.context, self.value.value, cname.as_ptr())
        };

        if raw.tag == TAG_EXCEPTION {
            Err(ExecutionError::Internal(format!(
                "Exception while getting property '{}'",
                name
            )))
        } else if raw.tag == TAG_UNDEFINED {
            Err(ExecutionError::Internal(format!(
                "Property '{}' not found",
                name
            )))
        } else {
            Ok(OwnedValueRef::new(self.value.context, raw))
        }
    }

    // Set a property on an object.
    // NOTE: this method takes ownership of the `JSValue`, so it must not be
    // freed later.
    unsafe fn set_property_raw(&self, name: &str, value: q::JSValue) -> Result<(), ExecutionError> {
        let cname = make_cstring(name)?;
        let ret = q::JS_SetPropertyStr(
            self.value.context.context,
            self.value.value,
            cname.as_ptr(),
            value,
        );
        if ret < 0 {
            Err(ExecutionError::Exception("Could not set property".into()))
        } else {
            Ok(())
        }
    }

    pub fn set_property(&self, name: &str, value: JsValue) -> Result<(), ExecutionError> {
        let qval = self.value.context.serialize_value(value)?;
        unsafe {
            // set_property_raw takes ownership, so we must prevent a free.
            self.set_property_raw(name, qval.extract())?;
        }
        Ok(())
    }
}

/*
type ModuleInit = dyn Fn(*mut q::JSContext, *mut q::JSModuleDef);

thread_local! {
    static NATIVE_MODULE_INIT: RefCell<Option<Box<ModuleInit>>> = RefCell::new(None);
}

unsafe extern "C" fn native_module_init(
    ctx: *mut q::JSContext,
    m: *mut q::JSModuleDef,
) -> ::std::os::raw::c_int {
    NATIVE_MODULE_INIT.with(|init| {
        let init = init.replace(None).unwrap();
        init(ctx, m);
    });
    0
}
*/

/// Wraps a quickjs context.
///
/// Cleanup of the context happens in drop.
pub struct ContextWrapper {
    runtime: *mut q::JSRuntime,
    pub(crate) context: *mut q::JSContext,
    /// Stores callback closures and quickjs data pointers.
    /// This array is write-only and only exists to ensure the lifetime of
    /// the closure.
    // A Mutex is used over a RefCell because it needs to be unwind-safe.
    callbacks: Mutex<Vec<(Box<WrappedCallback>, Box<q::JSValue>)>>,
}

impl Drop for ContextWrapper {
    fn drop(&mut self) {
        unsafe {
            q::JS_FreeContext(self.context);
            q::JS_FreeRuntime(self.runtime);
        }
    }
}

impl ContextWrapper {
    /// Initialize a wrapper by creating a JSRuntime and JSContext.
    pub fn new(memory_limit: Option<usize>) -> Result<Self, ContextError> {
        let runtime = unsafe { q::JS_NewRuntime() };
        if runtime.is_null() {
            return Err(ContextError::RuntimeCreationFailed);
        }

        // Configure memory limit if specified.
        if let Some(limit) = memory_limit {
            unsafe {
                q::JS_SetMemoryLimit(runtime, limit as _);
            }
        }

        let context = unsafe { q::JS_NewContext(runtime) };
        if context.is_null() {
            unsafe {
                q::JS_FreeRuntime(runtime);
            }
            return Err(ContextError::ContextCreationFailed);
        }

        // Initialize the promise resolver helper code.
        // This code is needed by Self::resolve_value
        let wrapper = Self {
            runtime,
            context,
            callbacks: Mutex::new(Vec::new()),
        };

        Ok(wrapper)
    }

    // See console standard: https://console.spec.whatwg.org
    pub fn set_console(&self, backend: Box<dyn ConsoleBackend>) -> Result<(), ExecutionError> {
        use crate::console::Level;

        self.add_callback("__console_write", move |args: Arguments| {
            let mut args = args.into_vec();

            if args.len() > 1 {
                let level_raw = args.remove(0);

                let level_opt = level_raw.as_str().and_then(|v| match v {
                    "trace" => Some(Level::Trace),
                    "debug" => Some(Level::Debug),
                    "log" => Some(Level::Log),
                    "info" => Some(Level::Info),
                    "warn" => Some(Level::Warn),
                    "error" => Some(Level::Error),
                    _ => None,
                });

                if let Some(level) = level_opt {
                    backend.log(level, args);
                }
            }
        })?;

        self.eval(
            r#"
            globalThis.console = {
                trace: (...args) => {
                    globalThis.__console_write("trace", ...args);
                },
                debug: (...args) => {
                    globalThis.__console_write("debug", ...args);
                },
                log: (...args) => {
                    globalThis.__console_write("log", ...args);
                },
                info: (...args) => {
                    globalThis.__console_write("info", ...args);
                },
                warn: (...args) => {
                    globalThis.__console_write("warn", ...args);
                },
                error: (...args) => {
                    globalThis.__console_write("error", ...args);
                },
            };
        "#,
        )?;

        Ok(())
    }

    /// Reset the wrapper by creating a new context.
    pub fn reset(self) -> Result<Self, ContextError> {
        unsafe {
            q::JS_FreeContext(self.context);
        };
        self.callbacks.lock().unwrap().clear();
        let context = unsafe { q::JS_NewContext(self.runtime) };
        if context.is_null() {
            return Err(ContextError::ContextCreationFailed);
        }

        let mut s = self;
        s.context = context;
        Ok(s)
    }

    pub fn serialize_value(&self, value: JsValue) -> Result<OwnedJsValue<'_>, ExecutionError> {
        let serialized = convert::serialize_value(self.context, value)?;
        Ok(OwnedJsValue::new(self, serialized))
    }

    // Deserialize a quickjs runtime value into a Rust value.
    pub(crate) fn to_value(&self, value: &q::JSValue) -> Result<JsValue, ValueError> {
        convert::deserialize_value(self.context, value)
    }

    /// Get the global object.
    pub fn global(&self) -> Result<OwnedJsObject<'_>, ExecutionError> {
        let global_raw = unsafe { q::JS_GetGlobalObject(self.context) };
        let global_ref = OwnedJsValue::new(self, global_raw);
        let global = global_ref.try_into_object()?;
        Ok(global)
    }

    /// Get the last exception from the runtime, and if present, convert it to a ExceptionError.
    pub(crate) fn get_exception(&self) -> Option<ExecutionError> {
        let value = unsafe {
            let raw = q::JS_GetException(self.context);
            OwnedJsValue::new(self, raw)
        };

        if value.is_null() {
            None
        } else if value.is_exception() {
            Some(ExecutionError::Internal(
                "Could get exception from runtime".into(),
            ))
        } else {
            match value.js_to_string() {
                Ok(strval) => {
                    if strval.contains("out of memory") {
                        Some(ExecutionError::OutOfMemory)
                    } else {
                        Some(ExecutionError::Exception(JsValue::String(strval)))
                    }
                }
                Err(e) => Some(e),
            }
        }
    }

    /// Returns `Result::Err` when an error ocurred.
    pub(crate) fn ensure_no_excpetion(&self) -> Result<(), ExecutionError> {
        if let Some(e) = self.get_exception() {
            Err(e)
        } else {
            Ok(())
        }
    }

    /// If the given value is a promise, run the event loop until it is
    /// resolved, and return the final value.
    fn resolve_value<'a>(
        &'a self,
        value: OwnedJsValue<'a>,
    ) -> Result<OwnedJsValue<'a>, ExecutionError> {
        if value.is_exception() {
            let err = self
                .get_exception()
                .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
            Err(err)
        } else if value.is_object() {
            let obj = value.try_into_object()?;
            if obj.is_promise()? {
                self.eval(
                    r#"
                    // Values:
                    //   - undefined: promise not finished
                    //   - false: error ocurred, __promiseError is set.
                    //   - true: finished, __promiseSuccess is set.
                    var __promiseResult = 0;
                    var __promiseValue = 0;

                    var __resolvePromise = function(p) {
                        p
                            .then(value => {
                                __promiseResult = true;
                                __promiseValue = value;
                            })
                            .catch(e => {
                                __promiseResult = false;
                                __promiseValue = e;
                            });
                    }
                "#,
                )?;

                let global = self.global()?;
                let resolver = global
                    .property_require("__resolvePromise")?
                    .try_into_function()?;

                // Call the resolver code that sets the result values once
                // the promise resolves.
                resolver.call(vec![obj.into_value()])?;

                loop {
                    let flag = unsafe {
                        let wrapper_mut = self as *const Self as *mut Self;
                        let ctx_mut = &mut (*wrapper_mut).context;
                        q::JS_ExecutePendingJob(self.runtime, ctx_mut)
                    };
                    if flag < 0 {
                        let e = self.get_exception().unwrap_or_else(|| {
                            ExecutionError::Exception("Unknown exception".into())
                        });
                        return Err(e);
                    }

                    // Check if promise is finished.
                    let res_val = global.property_require("__promiseResult")?;
                    if res_val.is_bool() {
                        let ok = res_val.to_bool()?;
                        let value = global.property_require("__promiseValue")?;

                        if ok {
                            return self.resolve_value(value);
                        } else {
                            let err_msg = value.js_to_string()?;
                            return Err(ExecutionError::Exception(JsValue::String(err_msg)));
                        }
                    }
                }
            } else {
                Ok(obj.into_value())
            }
        } else {
            Ok(value)
        }
    }

    /// Evaluate javascript code.
    pub fn eval<'a>(&'a self, code: &str) -> Result<OwnedJsValue<'a>, ExecutionError> {
        let filename = "script.js";
        let filename_c = make_cstring(filename)?;
        let code_c = make_cstring(code)?;

        let value_raw = unsafe {
            q::JS_Eval(
                self.context,
                code_c.as_ptr(),
                code.len() as _,
                filename_c.as_ptr(),
                q::JS_EVAL_TYPE_GLOBAL as i32,
            )
        };
        let value = OwnedJsValue::new(self, value_raw);
        self.resolve_value(value)
    }

    /*
    /// Call a constructor function.
    fn call_constructor<'a>(
        &'a self,
        function: OwnedJsValue<'a>,
        args: Vec<OwnedJsValue<'a>>,
    ) -> Result<OwnedJsValue<'a>, ExecutionError> {
        let mut qargs = args.iter().map(|arg| arg.value).collect::<Vec<_>>();

        let value_raw = unsafe {
            q::JS_CallConstructor(
                self.context,
                function.value,
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        let value = OwnedJsValue::new(self, value_raw);
        if value.is_exception() {
            let err = self
                .get_exception()
                .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
            Err(err)
        } else {
            Ok(value)
        }
    }
    */

    /// Call a JS function with the given arguments.
    pub fn call_function<'a>(
        &'a self,
        function: JsFunction<'a>,
        args: Vec<OwnedJsValue<'a>>,
    ) -> Result<OwnedJsValue<'a>, ExecutionError> {
        let ret = function.call(args)?;
        self.resolve_value(ret)
    }

    /// Helper for executing a callback closure.
    fn exec_callback<F>(
        context: *mut q::JSContext,
        argc: c_int,
        argv: *mut q::JSValue,
        callback: &impl Callback<F>,
    ) -> Result<q::JSValue, ExecutionError> {
        let result = std::panic::catch_unwind(|| {
            let arg_slice = unsafe { std::slice::from_raw_parts(argv, argc as usize) };

            let args = arg_slice
                .iter()
                .map(|raw| convert::deserialize_value(context, raw))
                .collect::<Result<Vec<_>, _>>()?;

            match callback.call(args) {
                Ok(Ok(result)) => {
                    let serialized = convert::serialize_value(context, result)?;
                    Ok(serialized)
                }
                // TODO: better error reporting.
                Ok(Err(e)) => Err(ExecutionError::Exception(JsValue::String(e))),
                Err(e) => Err(e.into()),
            }
        });

        match result {
            Ok(r) => r,
            Err(_e) => Err(ExecutionError::Internal("Callback panicked!".to_string())),
        }
    }

    /// Add a global JS function that is backed by a Rust function or closure.
    pub fn create_callback<'a, F>(
        &'a self,
        callback: impl Callback<F> + 'static,
    ) -> Result<JsFunction<'a>, ExecutionError> {
        let argcount = callback.argument_count() as i32;

        let context = self.context;
        let wrapper = move |argc: c_int, argv: *mut q::JSValue| -> q::JSValue {
            match Self::exec_callback(context, argc, argv, &callback) {
                Ok(value) => value,
                // TODO: better error reporting.
                Err(e) => {
                    let js_exception_value = match e {
                        ExecutionError::Exception(e) => e,
                        other => other.to_string().into(),
                    };
                    let js_exception =
                        convert::serialize_value(context, js_exception_value).unwrap();
                    unsafe {
                        q::JS_Throw(context, js_exception);
                    }

                    q::JSValue {
                        u: q::JSValueUnion { int32: 0 },
                        tag: TAG_EXCEPTION,
                    }
                }
            }
        };

        let (pair, trampoline) = unsafe { build_closure_trampoline(wrapper) };
        let data = (&*pair.1) as *const q::JSValue as *mut q::JSValue;
        self.callbacks.lock().unwrap().push(pair);

        let obj = unsafe {
            let f = q::JS_NewCFunctionData(self.context, trampoline, argcount, 0, 1, data);
            OwnedJsValue::new(self, f)
        };

        let f = obj.try_into_function()?;
        Ok(f)
    }

    pub fn add_callback<'a, F>(
        &'a self,
        name: &str,
        callback: impl Callback<F> + 'static,
    ) -> Result<(), ExecutionError> {
        let cfunc = self.create_callback(callback)?;
        let global = self.global()?;
        global.set_property(name, cfunc.into_value())?;
        Ok(())
    }
}
