use std::path::{PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;
use std::{
    slice,
    sync::Mutex,
};

use libquickjs_sys as q;

use crate::callback::WrappedCallback;
use crate::utils::{js_null_value, make_cstring};
#[cfg(feature = "bigint")]
use crate::value::{bigint::BigIntOrI64, BigInt};
use crate::{
    callback::{Arguments},
    console::ConsoleBackend,
    owned_value_ref::{OwnedObjectRef, OwnedValueRef},
    marshal::{serialize_value},
    timers::JsTimerRef,
    ContextError, ExecutionError, JsValue,
};

// JS_TAG_* constants from quickjs.
// For some reason bindgen does not pick them up.
#[cfg(feature = "bigint")]
pub const TAG_BIG_INT: i64 = -10;
pub const TAG_STRING: i64 = -7;
pub const TAG_MODULE: i64 = -3;
pub const TAG_OBJECT: i64 = -1;
pub const TAG_INT: i64 = 0;
pub const TAG_BOOL: i64 = 1;
pub const TAG_NULL: i64 = 2;
pub const TAG_UNDEFINED: i64 = 3;
pub const TAG_EXCEPTION: i64 = 6;
pub const TAG_FLOAT64: i64 = 7;

pub const TAG_INVALID: i64 = 100000;

/// Wraps a quickjs context.
///
/// Cleanup of the context happens in drop.
pub struct ContextWrapper {
    pub(crate) runtime: *mut q::JSRuntime,
    pub(crate) context: *mut q::JSContext,
    /// Used for QuickJS error reporting
    filename_hint: String,
    /// The directory path where modules are looked for.
    pub(crate) module_load_path: PathBuf,
    /// Stores callback closures and quickjs data pointers.
    /// This array is write-only and only exists to ensure the lifetime of
    /// the closure.
    ///
    /// A Mutex is used over a RefCell because it needs to be unwind-safe.
    pub(crate) callbacks: Mutex<Vec<(Box<WrappedCallback>, Box<q::JSValue>)>>,
    /// Timer references. Structured in a linked list
    /// A Mutex is used over a RefCell because it needs to be unwind-safe.
    pub(crate) timers: Arc<Mutex<JsTimerRef>>,
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
                q::JS_SetMemoryLimit(runtime, limit);
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
            filename_hint: "script.js".to_owned(),
            module_load_path: PathBuf::new(),
            timers: Arc::new(Mutex::new(None)),
        };

        Ok(wrapper)
    }

    /// See console standard: https://console.spec.whatwg.org
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
            false,
            false,
        )?;

        Ok(())
    }

    /// Reset the wrapper by creating a new context.
    pub fn reset(self) -> Result<Self, ContextError> {
        self.cancel_all_timers();
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

    pub fn serialize_value(&self, value: JsValue) -> Result<OwnedValueRef, ExecutionError> {
        let serialized = serialize_value(self.context, value)?;
        Ok(OwnedValueRef::wrap(self.context, serialized))
    }

    /// QuickJS error reporting includes a script filename.
    /// Set the file name to be reported here.
    pub fn set_filename_hint(&mut self, filename: String) {
        self.filename_hint = filename;
    }

    /// Get the global object.
    pub fn global(&self) -> Result<OwnedObjectRef, ExecutionError> {
        let global_raw = unsafe { q::JS_GetGlobalObject(self.context) };
        let global_ref = OwnedValueRef::wrap(self.context, global_raw);
        let global = OwnedObjectRef::new(global_ref)?;
        Ok(global)
    }

    /// Get the last exception from the runtime, and if present, convert it to a ExceptionError.
    fn get_exception(&self) -> Option<ExecutionError> {
        let raw = unsafe { q::JS_GetException(self.context) };
        let value = OwnedValueRef::wrap(self.context, raw);

        if value.is_null() {
            None
        } else {
            let err = if value.is_exception() {
                ExecutionError::Internal("Could get exception from runtime".into())
            } else {
                match value.to_string() {
                    Ok(strval) => {
                        if strval.contains("out of memory") {
                            ExecutionError::OutOfMemory
                        } else {
                            ExecutionError::Exception(JsValue::String(strval))
                        }
                    }
                    Err(_) => ExecutionError::Internal("Unknown exception".into()),
                }
            };
            Some(err)
        }
    }

    fn resolve_promise_prepare(
        &self,
        value: OwnedValueRef,
    ) -> Result<(bool, OwnedValueRef), ExecutionError> {
        if !value.is_object() {
            return Ok((false, value));
        }

        let obj = OwnedObjectRef::new(value)?;
        if !obj.is_promise()? {
            return Ok((false, obj.into_value()));
        }
        // Values:
        //   - undefined: promise not finished
        //   - false: error ocurred, __promiseError is set.
        //   - true: finished, __promiseSuccess is set.
        self.eval(
            r#"
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
            false,
            false,
        )?;

        let global = self.global()?;
        let resolver = global.property("__resolvePromise")?;

        // Call the resolver code that sets the result values once
        // the promise resolves.
        Ok((
            true,
            self.call_function(resolver, vec![obj.into_value()], true)?,
        ))
    }

    /// If the given value is a promise, run the event loop until it is
    /// resolved, and return the final value.
    fn resolve_value(
        &self,
        value: OwnedValueRef,
    ) -> Result<OwnedValueRef, ExecutionError> {
        if value.is_exception() {
            let err = self
                .get_exception()
                .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
            return Err(err);
        }

        let (is_promise, value) = self.resolve_promise_prepare(value)?;
        let mut running = true;

        while running {
            running = self.await_timers();
            let flag = unsafe {
                let ctx_mut = NonNull::new_unchecked(*&self.context);
                let ctx_mut = &mut ctx_mut.as_ptr();
                q::JS_ExecutePendingJob(self.runtime, ctx_mut)
            };
            if flag < 0 {
                let e = self
                    .get_exception()
                    .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
                return Err(e);
            } else if flag > 0 {
                running = true;
                continue;
            }

            // flag==0 means no progress. Check timers and if the return value promise has resolved

            if is_promise {
                let global = self.global()?;
                // Check if promise is finished.
                let res_val = global.property("__promiseResult")?;
                if res_val.is_bool() {
                    let ok = res_val.to_bool()?;
                    let value = global.property("__promiseValue")?;

                    if ok {
                        return self.resolve_value(value);
                    } else {
                        let err_msg = value.to_string()?;
                        return Err(ExecutionError::Exception(JsValue::String(err_msg)));
                    }
                }
            }
        }

        Ok(value)
    }

    /// Evaluate javascript code.
    pub fn eval(
        &self,
        code: &str,
        compile_only: bool,
        module: bool,
    ) -> Result<OwnedValueRef, ExecutionError> {
        let filename_c = make_cstring(&self.filename_hint[..])?;
        let code_c = make_cstring(code)?;

        let mut flags = 0;
        if module {
            flags |= q::JS_EVAL_TYPE_MODULE as i32
        }
        if compile_only {
            flags |= q::JS_EVAL_FLAG_COMPILE_ONLY as i32
        }
        let ctx = self.context;
        if ctx.is_null() {
            return Err(ExecutionError::InputWithZeroBytes);
        }
        let value_raw =
            unsafe { q::JS_Eval(ctx, code_c.as_ptr(), code.len(), filename_c.as_ptr(), flags) };
        let value = OwnedValueRef::wrap(self.context, value_raw);
        if compile_only {
            if value.is_exception() {
                let err = self
                    .get_exception()
                    .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
                return Err(err);
            }
            if (module || compile_only) && value.value.tag != TAG_MODULE {
                return Err(ExecutionError::Internal(format!(
                    "Expected module value. Got {:?}",
                    value
                )));
            }
            Ok(value)
        } else {
            self.resolve_value(value)
        }
    }

    /// Converts a QuickJS value into bytecode. Called by compile().
    pub(crate) fn value_to_bytecode(
        &self,
        value: OwnedValueRef,
    ) -> Result<Vec<u8>, ExecutionError> {
        let raw_value = unsafe {
            let mut len = 0;
            let buf = q::JS_WriteObject(
                self.context,
                &mut len,
                value.value.clone(),
                q::JS_WRITE_OBJ_BYTECODE as i32,
            );
            let data = slice::from_raw_parts::<u8>(buf, len).to_vec();
            q::js_free(self.context, buf as *mut std::ffi::c_void);
            data
        };
        Ok(raw_value)
    }

    /// This method is similar to [`eval()`] but executes byte code, produced by [`compile()`] instead.
    pub fn run_bytecode(&self, code: &[u8]) -> Result<OwnedValueRef, ExecutionError> {
        /// allow function/module
        const BYTECODE: u32 = q::JS_READ_OBJ_BYTECODE;
        /// avoid duplicating 'buf' data
        const ROM_DATA: u32 = q::JS_READ_OBJ_ROM_DATA;

        let raw_value = unsafe {
            q::JS_ReadObject(
                self.context,
                code.as_ptr(),
                code.len(),
                (BYTECODE | ROM_DATA) as i32,
            )
        };

        if raw_value.tag != TAG_MODULE {
            let value = OwnedValueRef::wrap(self.context, raw_value);
            return Err(ExecutionError::Internal(format!(
                "Expected module value. Got {:?}",
                value
            )));
        }

        let raw_value = unsafe {
            if q::JS_ResolveModule(self.context, raw_value) != 0 {
                let err = self
                    .get_exception()
                    .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
                return Err(err);
            }
            if q::js_module_set_import_meta(self.context, raw_value, 0, 1) != 0 {
                let err = self
                    .get_exception()
                    .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
                return Err(err);
            }
            q::JS_EvalFunction(self.context, raw_value)
        };

        let value = OwnedValueRef::wrap(self.context, raw_value);
        self.resolve_value(value)
    }

    /*
    /// Call a constructor function.
    fn call_constructor<'a>(
        &'a self,
        function: OwnedValueRef<'a>,
        args: Vec<OwnedValueRef<'a>>,
    ) -> Result<OwnedValueRef<'a>, ExecutionError> {
        let mut qargs = args.iter().map(|arg| arg.value).collect::<Vec<_>>();

        let value_raw = unsafe {
            q::JS_CallConstructor(
                self.context,
                function.value,
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        let value = OwnedValueRef::new(self, value_raw);
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

    /// Call a JS function with the given arguments:
    /// * `function` The function to call
    /// * `args` The function arguments
    /// * `resolve_value` Set to true if you want to resolve the returned value.
    ///   Never set this if called from within [`resolve_value`], it will deadlock.
    ///   Setting this is only really necessary if the returned value is a promise that should be awaited for.
    pub fn call_function(
        &self,
        function: OwnedValueRef,
        args: Vec<OwnedValueRef>,
        resolve_value: bool,
    ) -> Result<OwnedValueRef, ExecutionError> {
        let mut qargs = args.iter().map(|arg| arg.value).collect::<Vec<_>>();

        let qres_raw = unsafe {
            q::JS_Call(
                self.context,
                function.value,
                js_null_value(),
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        let qres = OwnedValueRef::wrap(self.context, qres_raw);
        if resolve_value {
            self.resolve_value(qres)
        } else {
            Ok(qres)
        }
    }
}
