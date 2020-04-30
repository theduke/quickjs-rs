use std::{
    collections::HashMap,
    ffi::CString,
    os::raw::{c_char, c_int, c_void},
    sync::Mutex,
};

use libquickjs_sys as q;

#[cfg(feature = "bigint")]
use crate::value::{bigint::BigIntOrI64, BigInt};
use crate::{
    callback::{Arguments, Callback},
    console::ConsoleBackend,
    droppable_value::DroppableValue,
    ContextError, ExecutionError, JsValue, ValueError,
};

// JS_TAG_* constants from quickjs.
// For some reason bindgen does not pick them up.
#[cfg(feature = "bigint")]
const TAG_BIG_INT: i64 = -10;
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

#[cfg(feature = "chrono")]
fn js_date_constructor(context: *mut q::JSContext) -> q::JSValue {
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
fn js_create_bigint_function(context: *mut q::JSContext) -> q::JSValue {
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

/// Serialize a Rust value into a quickjs runtime value.
fn serialize_value(context: *mut q::JSContext, value: JsValue) -> Result<q::JSValue, ValueError> {
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
            let qval = unsafe {
                q::JS_NewStringLen(context, val.as_ptr() as *const c_char, val.len() as _)
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
                let qvalue = match serialize_value(context, value) {
                    Ok(qval) => qval,
                    Err(e) => {
                        // Make sure to free the array if a individual element
                        // fails.
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
                        qvalue,
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

                let qvalue = serialize_value(context, value).map_err(|e| {
                    // Free the object if a property failed.
                    unsafe {
                        free_value(context, obj);
                    }
                    e
                })?;

                let ret = unsafe {
                    q::JS_DefinePropertyValueStr(
                        context,
                        obj,
                        ckey.as_ptr(),
                        qvalue,
                        q::JS_PROP_C_W_E as i32,
                    )
                };
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
        #[cfg(feature = "chrono")]
        JsValue::Date(datetime) => {
            let date_constructor = js_date_constructor(context);

            let f = datetime.timestamp_millis() as f64;

            let timestamp = q::JSValue {
                u: q::JSValueUnion { float64: f },
                tag: TAG_FLOAT64,
            };

            let mut args = vec![timestamp];

            let value = unsafe {
                q::JS_CallConstructor(
                    context,
                    date_constructor,
                    args.len() as i32,
                    args.as_mut_ptr(),
                )
            };
            unsafe {
                free_value(context, date_constructor);
            }

            if value.tag != TAG_OBJECT {
                return Err(ValueError::Internal(
                    "Could not construct Date object".into(),
                ));
            }
            value
        }
        #[cfg(feature = "bigint")]
        JsValue::BigInt(int) => match int.inner {
            BigIntOrI64::Int(int) => unsafe { q::JS_NewBigInt64(context, int) },
            BigIntOrI64::BigInt(bigint) => {
                let bigint_string = bigint.to_str_radix(10);
                let s = unsafe {
                    q::JS_NewStringLen(
                        context,
                        bigint_string.as_ptr() as *const c_char,
                        bigint_string.len(),
                    )
                };
                let s = DroppableValue::new(s, |&mut s| unsafe {
                    free_value(context, s);
                });
                if (*s).tag != TAG_STRING {
                    return Err(ValueError::Internal(
                        "Could not construct String object needed to create BigInt object".into(),
                    ));
                }

                let mut args = vec![*s];

                let bigint_function = js_create_bigint_function(context);
                let bigint_function =
                    DroppableValue::new(bigint_function, |&mut bigint_function| unsafe {
                        free_value(context, bigint_function);
                    });
                let js_bigint = unsafe {
                    q::JS_Call(
                        context,
                        *bigint_function,
                        js_null_value(),
                        1,
                        args.as_mut_ptr(),
                    )
                };

                if js_bigint.tag != TAG_BIG_INT {
                    return Err(ValueError::Internal(
                        "Could not construct BigInt object".into(),
                    ));
                }

                js_bigint
            }
        },
        JsValue::__NonExhaustive => unreachable!(),
    };
    Ok(v)
}

fn deserialize_array(
    context: *mut q::JSContext,
    raw_value: &q::JSValue,
) -> Result<JsValue, ValueError> {
    assert_eq!(raw_value.tag, TAG_OBJECT);

    let length_name = make_cstring("length")?;

    let len_raw = unsafe { q::JS_GetPropertyStr(context, *raw_value, length_name.as_ptr()) };

    let len_res = deserialize_value(context, &len_raw);
    unsafe { free_value(context, len_raw) };
    let len = match len_res? {
        JsValue::Int(x) => x,
        _ => {
            return Err(ValueError::Internal(
                "Could not determine array length".into(),
            ));
        }
    };

    let mut values = Vec::new();
    for index in 0..(len as usize) {
        let value_raw = unsafe { q::JS_GetPropertyUint32(context, *raw_value, index as u32) };
        if value_raw.tag == TAG_EXCEPTION {
            return Err(ValueError::Internal("Could not build array".into()));
        }
        let value_res = deserialize_value(context, &value_raw);
        unsafe { free_value(context, value_raw) };

        let value = value_res?;
        values.push(value);
    }

    Ok(JsValue::Array(values))
}

fn deserialize_object(context: *mut q::JSContext, obj: &q::JSValue) -> Result<JsValue, ValueError> {
    assert_eq!(obj.tag, TAG_OBJECT);

    let mut properties: *mut q::JSPropertyEnum = std::ptr::null_mut();
    let mut count: u32 = 0;

    let flags = (q::JS_GPN_STRING_MASK | q::JS_GPN_SYMBOL_MASK | q::JS_GPN_ENUM_ONLY) as i32;
    let ret =
        unsafe { q::JS_GetOwnPropertyNames(context, &mut properties, &mut count, *obj, flags) };
    if ret != 0 {
        return Err(ValueError::Internal(
            "Could not get object properties".into(),
        ));
    }

    // TODO: refactor into a more Rust-idiomatic iterator wrapper.
    let properties = DroppableValue::new(properties, |&mut properties| {
        for index in 0..count {
            let prop = unsafe { properties.offset(index as isize) };
            unsafe {
                q::JS_FreeAtom(context, (*prop).atom);
            }
        }
        unsafe {
            q::js_free(context, properties as *mut std::ffi::c_void);
        }
    });

    let mut map = HashMap::new();
    for index in 0..count {
        let prop = unsafe { (*properties).offset(index as isize) };
        let raw_value = unsafe { q::JS_GetPropertyInternal(context, *obj, (*prop).atom, *obj, 0) };
        if raw_value.tag == TAG_EXCEPTION {
            return Err(ValueError::Internal("Could not get object property".into()));
        }

        let value_res = deserialize_value(context, &raw_value);
        unsafe {
            free_value(context, raw_value);
        }
        let value = value_res?;

        let key_value = unsafe { q::JS_AtomToString(context, (*prop).atom) };
        if key_value.tag == TAG_EXCEPTION {
            return Err(ValueError::Internal(
                "Could not get object property name".into(),
            ));
        }

        let key_res = deserialize_value(context, &key_value);
        unsafe {
            free_value(context, key_value);
        }
        let key = match key_res? {
            JsValue::String(s) => s,
            _ => {
                return Err(ValueError::Internal("Could not get property name".into()));
            }
        };
        map.insert(key, value);
    }

    Ok(JsValue::Object(map))
}

fn deserialize_value(
    context: *mut q::JSContext,
    value: &q::JSValue,
) -> Result<JsValue, ValueError> {
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
            let ptr = unsafe { q::JS_ToCStringLen2(context, std::ptr::null_mut(), *r, 0) };

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
                deserialize_array(context, r)
            } else {
                #[cfg(feature = "chrono")]
                {
                    use chrono::offset::TimeZone;

                    let date_constructor = js_date_constructor(context);
                    let is_date = unsafe { q::JS_IsInstanceOf(context, *r, date_constructor) > 0 };

                    if is_date {
                        let getter = unsafe {
                            q::JS_GetPropertyStr(
                                context,
                                *r,
                                std::ffi::CStr::from_bytes_with_nul(b"getTime\0")
                                    .unwrap()
                                    .as_ptr(),
                            )
                        };
                        assert_eq!(getter.tag, TAG_OBJECT);

                        let timestamp_raw =
                            unsafe { q::JS_Call(context, getter, *r, 0, std::ptr::null_mut()) };
                        unsafe {
                            free_value(context, getter);
                            free_value(context, date_constructor);
                        };

                        let res = if timestamp_raw.tag != TAG_FLOAT64 {
                            Err(ValueError::Internal(
                                "Could not convert 'Date' instance to timestamp".into(),
                            ))
                        } else {
                            let f = unsafe { timestamp_raw.u.float64 } as i64;
                            let datetime = chrono::Utc.timestamp_millis(f);
                            Ok(JsValue::Date(datetime))
                        };
                        return res;
                    } else {
                        unsafe { free_value(context, date_constructor) };
                    }
                }

                deserialize_object(context, r)
            }
        }
        // BigInt
        #[cfg(feature = "bigint")]
        TAG_BIG_INT => {
            let mut int: i64 = 0;
            let ret = unsafe { q::JS_ToBigInt64(context, &mut int, *r) };
            if ret == 0 {
                Ok(JsValue::BigInt(BigInt {
                    inner: BigIntOrI64::Int(int),
                }))
            } else {
                let ptr = unsafe { q::JS_ToCStringLen2(context, std::ptr::null_mut(), *r, 0) };

                if ptr.is_null() {
                    return Err(ValueError::Internal(
                        "Could not convert BigInt to string: got a null pointer".into(),
                    ));
                }

                let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
                let bigint = num_bigint::BigInt::parse_bytes(cstr.to_bytes(), 10).unwrap();

                // Free the c string.
                unsafe { q::JS_FreeCString(context, ptr) };

                Ok(JsValue::BigInt(BigInt {
                    inner: BigIntOrI64::BigInt(bigint),
                }))
            }
        }
        x => Err(ValueError::Internal(format!(
            "Unhandled JS_TAG value: {}",
            x
        ))),
    }
}

/// Helper for creating CStrings.
fn make_cstring(value: impl Into<Vec<u8>>) -> Result<CString, ValueError> {
    CString::new(value).map_err(ValueError::StringWithZeroBytes)
}

/// Helper to construct null JsValue
fn js_null_value() -> q::JSValue {
    q::JSValue {
        u: q::JSValueUnion { int32: 0 },
        tag: TAG_NULL,
    }
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
            free_value(self.context.context, self.value);
        }
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
            _ => write!(f, "?"),
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
    //unsafe fn into_inner(mut self) -> q::JSValue {
    //let v = self.value;
    //self.value = q::JSValue {
    //u: q::JSValueUnion { int32: 0 },
    //tag: TAG_NULL,
    //};
    //v
    //}

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
            free_value(self.value.context.context, raw);
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

    pub fn serialize_value(&self, value: JsValue) -> Result<OwnedValueRef<'_>, ExecutionError> {
        let serialized = serialize_value(self.context, value)?;
        Ok(OwnedValueRef::new(self, serialized))
    }

    // Deserialize a quickjs runtime value into a Rust value.
    fn to_value(&self, value: &q::JSValue) -> Result<JsValue, ValueError> {
        deserialize_value(self.context, value)
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

    /// If the given value is a promise, run the event loop until it is
    /// resolved, and return the final value.
    fn resolve_value<'a>(
        &'a self,
        value: OwnedValueRef<'a>,
    ) -> Result<OwnedValueRef<'a>, ExecutionError> {
        if value.is_exception() {
            let err = self
                .get_exception()
                .unwrap_or_else(|| ExecutionError::Exception("Unknown exception".into()));
            Err(err)
        } else if value.is_object() {
            let obj = OwnedObjectRef::new(value)?;
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
                let resolver = global.property("__resolvePromise")?;

                // Call the resolver code that sets the result values once
                // the promise resolves.
                self.call_function(resolver, vec![obj.into_value()])?;

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
            } else {
                Ok(obj.into_value())
            }
        } else {
            Ok(value)
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
                code.len() as _,
                filename_c.as_ptr(),
                q::JS_EVAL_TYPE_GLOBAL as i32,
            )
        };
        let value = OwnedValueRef::new(self, value_raw);
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

    /// Call a JS function with the given arguments.
    pub fn call_function<'a>(
        &'a self,
        function: OwnedValueRef<'a>,
        args: Vec<OwnedValueRef<'a>>,
    ) -> Result<OwnedValueRef<'a>, ExecutionError> {
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
        let qres = OwnedValueRef::new(self, qres_raw);
        self.resolve_value(qres)
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
                .map(|raw| deserialize_value(context, raw))
                .collect::<Result<Vec<_>, _>>()?;

            match callback.call(args) {
                Ok(Ok(result)) => {
                    let serialized = serialize_value(context, result)?;
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
    ) -> Result<q::JSValue, ExecutionError> {
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
                    let js_exception = serialize_value(context, js_exception_value).unwrap();
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

        let cfunc =
            unsafe { q::JS_NewCFunctionData(self.context, trampoline, argcount, 0, 1, data) };
        if cfunc.tag != TAG_OBJECT {
            return Err(ExecutionError::Internal("Could not create callback".into()));
        }

        Ok(cfunc)
    }

    pub fn add_callback<'a, F>(
        &'a self,
        name: &str,
        callback: impl Callback<F> + 'static,
    ) -> Result<(), ExecutionError> {
        let cfunc = self.create_callback(callback)?;
        let global = self.global()?;
        unsafe {
            global.set_property_raw(name, cfunc)?;
        }
        Ok(())
    }
}
