use std::{
    ffi::CString,
    os::raw::{c_int, c_void},
    sync::Mutex,
};

use libquickjs_sys as q;

use crate::{callback::Callback, ContextError, ExecutionError, JsValue, ValueError};

// JS_TAG_* constants from quickjs.
// For some reason bindgen does not pick them up.
const TAG_STRING: i64 = -7;
const TAG_OBJECT: i64 = -1;
const TAG_INT: i64 = 0;
const TAG_BOOL: i64 = 1;
const TAG_NULL: i64 = 2;
const TAG_UNDEFINED: i64 = 3;
const TAG_EXCEPTION: i64 = 6;
const TAG_FLOAT64: i64 = 7;

/// Free a JSValue.
/// This function is the equivalent of JS_FreeValue from quickjs, which can not
/// be used due to being `static inline`.
unsafe fn free_value(context: *mut q::JSContext, value: q::JSValue) {
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
///
/// TODO: use catch_unwind to prevent pancis.
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
            free_value(self.context.context, self.value);
        }
    }
}

impl<'a> OwnedValueRef<'a> {
    pub fn new(context: &'a ContextWrapper, value: q::JSValue) -> Self {
        Self { context, value }
    }

    /// Get the inner JSValue without freeing in drop.
    ///
    /// Unsafe because the caller is responsible for freeing the value.
    unsafe fn into_inner(mut self) -> q::JSValue {
        let v = self.value;
        self.value = q::JSValue {
            u: q::JSValueUnion { int32: 0 },
            tag: TAG_NULL,
        };
        v
    }

    pub fn is_null(&self) -> bool {
        self.value.tag == TAG_NULL
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

    pub fn property(&'a self, name: &str) -> Result<OwnedValueRef<'a>, ExecutionError> {
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

    // pub fn set_property(&self, name: &str, value: JsValue) -> Result<(), ExecutionError> {
    //     let qval = self.value.context.serialize_value(value)?;
    //     unsafe { self.set_property_raw(name, qval.value) }
    // }
}

/// Wraps a quickjs context.
///
/// Cleanup of the context happens in drop.
pub struct ContextWrapper {
    runtime: *mut q::JSRuntime,
    context: *mut q::JSContext,
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
                q::JS_SetMemoryLimit(runtime, limit);
            }
        }

        let context = unsafe { q::JS_NewContext(runtime) };
        if context.is_null() {
            return Err(ContextError::ContextCreationFailed);
        }

        Ok(Self {
            runtime,
            context,
            callbacks: Mutex::new(Vec::new()),
        })
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

    /// Serialize a Rust value into a quickjs runtime value.
    pub fn serialize_value(&self, value: JsValue) -> Result<OwnedValueRef<'_>, ValueError> {
        let context = self.context;
        let v = match value {
            JsValue::Null => q::JSValue {
                u: q::JSValueUnion { int32: 0 },
                tag: TAG_NULL,
            },
            JsValue::Bool(flag) => q::JSValue {
                u: q::JSValueUnion {
                    int32: if flag { 1 } else { 0 },
                },
                tag: TAG_BOOL,
            },
            JsValue::Int(val) => q::JSValue {
                u: q::JSValueUnion { int32: val },
                tag: TAG_INT,
            },
            JsValue::Float(val) => q::JSValue {
                u: q::JSValueUnion { float64: val },
                tag: TAG_FLOAT64,
            },
            JsValue::String(val) => {
                let len = val.len();
                let qval = unsafe {
                    q::JS_NewStringLen(
                        context,
                        val.as_ptr() as *const i8,
                        len as std::os::raw::c_int,
                    )
                };

                if qval.tag == TAG_EXCEPTION {
                    return Err(ValueError::Internal(
                        "Could not create string in runtime".into(),
                    ));
                }

                qval
            }
            JsValue::Array(values) => {
                // Allocate a new array in the runtime.
                let arr = unsafe { q::JS_NewArray(context) };
                if arr.tag == TAG_EXCEPTION {
                    return Err(ValueError::Internal(
                        "Could not create array in runtime".into(),
                    ));
                }

                for (index, value) in values.into_iter().enumerate() {
                    let qvalue = match self.serialize_value(value) {
                        Ok(qval) => qval,
                        Err(e) => {
                            // Make sure to free the array if a individual
                            // element fails.
                            unsafe {
                                free_value(context, arr);
                            }
                            return Err(e);
                        }
                    };

                    let ret = unsafe {
                        q::JS_DefinePropertyValueUint32(
                            context,
                            arr,
                            index as u32,
                            qvalue.value,
                            q::JS_PROP_C_W_E as i32,
                        )
                    };
                    if ret < 0 {
                        // Make sure to free the array if a individual
                        // element fails.
                        unsafe {
                            free_value(context, arr);
                        }
                        return Err(ValueError::Internal(
                            "Could not append element to array".into(),
                        ));
                    }
                }
                arr
            }
            JsValue::Object(map) => {
                let obj = unsafe { q::JS_NewObject(context) };
                if obj.tag == TAG_EXCEPTION {
                    return Err(ValueError::Internal("Could not create object".into()));
                }

                for (key, value) in map {
                    let ckey = make_cstring(key)?;

                    let qvalue = self.serialize_value(value).map_err(|e| {
                        // Free the object if a property failed.
                        unsafe {
                            free_value(context, obj);
                        }
                        e
                    })?;

                    let ret =
                        unsafe { q::JS_SetPropertyStr(context, obj, ckey.as_ptr(), qvalue.value) };
                    if ret < 0 {
                        // Free the object if a property failed.
                        unsafe {
                            free_value(context, obj);
                        }
                        return Err(ValueError::Internal(
                            "Could not add add property to object".into(),
                        ));
                    }
                }

                obj
            }
        };
        Ok(OwnedValueRef::new(self, v))
    }

    // Deserialize a quickjs runtime value into a Rust value.
    fn to_value(&self, value: &q::JSValue) -> Result<JsValue, ValueError> {
        let context = self.context;
        let r = value;

        match r.tag {
            // Int.
            TAG_INT => {
                let val = unsafe { r.u.int32 };
                Ok(JsValue::Int(val))
            }
            // Bool.
            TAG_BOOL => {
                let raw = unsafe { r.u.int32 };
                let val = raw > 0;
                Ok(JsValue::Bool(val))
            }
            // Null.
            TAG_NULL => Ok(JsValue::Null),
            // Undefined.
            TAG_UNDEFINED => Ok(JsValue::Null),
            // Float.
            TAG_FLOAT64 => {
                let val = unsafe { r.u.float64 };
                Ok(JsValue::Float(val))
            }
            // String.
            TAG_STRING => {
                let ptr = unsafe {
                    q::JS_ToCStringLen(context, std::ptr::null::<i32>() as *mut i32, *r, 0)
                };

                if ptr.is_null() {
                    return Err(ValueError::Internal(
                        "Could not convert string: got a null pointer".into(),
                    ));
                }

                let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };

                let s = cstr
                    .to_str()
                    .map_err(ValueError::InvalidString)?
                    .to_string();

                // Free the c string.
                unsafe { q::JS_FreeCString(context, ptr) };

                Ok(JsValue::String(s))
            }
            // Object.
            TAG_OBJECT => {
                let is_array = unsafe { q::JS_IsArray(context, *r) } > 0;
                if is_array {
                    let length_name = make_cstring("length")?;

                    let len_value = unsafe {
                        let raw = q::JS_GetPropertyStr(context, *r, length_name.as_ptr());
                        let wrapped = OwnedValueRef::new(self, raw);
                        wrapped.to_value()?
                    };
                    let len = if let JsValue::Int(x) = len_value {
                        x
                    } else {
                        return Err(ValueError::Internal(
                            "Could not determine arrya length".into(),
                        ));
                    };

                    let mut values = Vec::new();
                    for index in 0..(len as usize) {
                        let value_raw =
                            unsafe { q::JS_GetPropertyUint32(context, *r, index as u32) };
                        let value_ref = OwnedValueRef::new(self, value_raw);
                        if value_ref.value.tag == TAG_EXCEPTION {
                            return Err(ValueError::Internal("Could not build array".into()));
                        }
                        let value = value_ref.to_value()?;
                        values.push(value);
                    }

                    Ok(JsValue::Array(values))
                } else {
                    Err(ValueError::Internal("Unsupported JS type: Object".into()))
                }
            }
            x => Err(ValueError::Internal(format!(
                "Unhandled JS_TAG value: {}",
                x
            ))),
        }
    }

    /// Get the global object.
    pub fn global(&self) -> Result<OwnedObjectRef<'_>, ExecutionError> {
        let global_raw = unsafe { q::JS_GetGlobalObject(self.context) };
        let global_ref = OwnedValueRef::new(self, global_raw);
        let global = OwnedObjectRef::new(global_ref)?;
        Ok(global)
    }

    /// Get the last exception from the runtime, and if present, convert it to a ExceptionError.
    fn get_exception(&self) -> Option<ExecutionError> {
        let raw = unsafe { q::JS_GetException(self.context) };
        let value = OwnedValueRef::new(self, raw);

        if value.is_null() {
            None
        } else {
            let err = if value.is_exception() {
                ExecutionError::Internal("Could get exception from runtime".into())
            } else {
                match value.to_value() {
                    Ok(e) => ExecutionError::Exception(e),
                    Err(_) => match value.to_string() {
                        Ok(strval) => {
                            if strval.contains("out of memory") {
                                ExecutionError::OutOfMemory
                            } else {
                                ExecutionError::Exception(JsValue::String(strval))
                            }
                        }
                        Err(_) => ExecutionError::Internal("Unknown exception".into()),
                    },
                }
            };
            Some(err)
        }
    }

    /// Evaluate javascript code.
    pub fn eval<'a>(&'a self, code: &str) -> Result<OwnedValueRef<'a>, ExecutionError> {
        let filename = "script.js";
        let filename_c = make_cstring(filename)?;
        let code_c = make_cstring(code)?;

        let value_raw = unsafe {
            q::JS_Eval(
                self.context,
                code_c.as_ptr(),
                code.len(),
                filename_c.as_ptr(),
                q::JS_EVAL_TYPE_GLOBAL as i32,
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

    /// Call a JS function with the given arguments.
    pub fn call_function<'a>(
        &'a self,
        function: OwnedValueRef<'a>,
        args: Vec<OwnedValueRef<'a>>,
    ) -> Result<OwnedValueRef<'a>, ExecutionError> {
        let mut qargs = args.iter().map(|arg| arg.value).collect::<Vec<_>>();

        let n = q::JSValue {
            u: q::JSValueUnion { int32: 0 },
            tag: TAG_NULL,
        };

        let qres_raw = unsafe {
            q::JS_Call(
                self.context,
                function.value,
                n,
                qargs.len() as i32,
                qargs.as_mut_ptr(),
            )
        };
        let qres = OwnedValueRef::new(self, qres_raw);

        if qres.is_exception() {
            let err = self
                .get_exception()
                .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
            Err(err)
        } else {
            Ok(qres)
        }
    }

    /// Helper for executing a callback closure.
    fn exec_callback<'a, F>(
        &'a self,
        argc: c_int,
        argv: *mut q::JSValue,
        callback: &impl Callback<F>,
    ) -> Result<OwnedValueRef<'a>, ExecutionError> {
        let result = std::panic::catch_unwind(|| {
            let arg_slice = unsafe { std::slice::from_raw_parts(argv, argc as usize) };

            let args = arg_slice
                .iter()
                .map(|raw| self.to_value(raw))
                .collect::<Result<Vec<_>, _>>()?;

            match callback.call(args) {
                Ok(Ok(result)) => {
                    let serialized = self.serialize_value(result)?;
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
    pub fn add_callback<'a, F>(
        &'a self,
        name: &str,
        callback: impl Callback<F> + 'static,
    ) -> Result<(), ExecutionError> {
        let self_ptr = self as *const ContextWrapper;

        let argcount = callback.argument_count() as i32;

        let wrapper = move |argc: c_int, argv: *mut q::JSValue| -> q::JSValue {
            let ctx: &ContextWrapper = unsafe { &*self_ptr };

            match ctx.exec_callback(argc, argv, &callback) {
                Ok(value) => unsafe { value.into_inner() },
                // TODO: better error reporting.
                Err(e) => {
                    let js_exception_value = match e {
                        ExecutionError::Exception(e) => e,
                        other => other.to_string().into(),
                    };
                    let js_exception = ctx.serialize_value(js_exception_value).unwrap();
                    unsafe {
                        q::JS_Throw(ctx.context, js_exception.into_inner());
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

        let cfunc =
            unsafe { q::JS_NewCFunctionData(self.context, trampoline, argcount, 0, 1, data) };
        if cfunc.tag != TAG_OBJECT {
            return Err(ExecutionError::Internal("Could not create callback".into()));
        }

        let global = self.global()?;
        unsafe {
            global.set_property_raw(name, cfunc)?;
        }

        Ok(())
    }
}
