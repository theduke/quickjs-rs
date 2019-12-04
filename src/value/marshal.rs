//! # Serialize / Deserialize from/to a C-QuickJS JSValue and a Rust type JsValue
#[cfg(feature = "bigint")]
use crate::bigint::{BigInt, BigIntOrI64};
#[cfg(feature = "bigint")]
use crate::bindings::TAG_BIG_INT;
use crate::bindings::{
    TAG_BOOL, TAG_EXCEPTION, TAG_FLOAT64, TAG_INT, TAG_NULL, TAG_OBJECT, TAG_STRING, TAG_UNDEFINED,
};
use crate::owned_value_ref::OwnedValueRef;
use crate::utils::{free_value, make_cstring};
#[cfg(feature = "chrono")]
use crate::utils::js_date_constructor;
#[cfg(feature = "bigint")]
use crate::utils::js_create_bigint_function;

use crate::{JsValue, ValueError};

use libquickjs_sys as q;

use std::collections::HashMap;
use std::os::raw::c_char;

/// Serialize a Rust value into a quickjs runtime value.
pub fn serialize_value(
    context: *mut q::JSContext,
    value: JsValue,
) -> Result<q::JSValue, ValueError> {
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
            let qval =
                unsafe { q::JS_NewStringLen(context, val.as_ptr() as *const c_char, val.len()) };

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
                let s = OwnedValueRef::wrap(context, s);
                if (*s).tag != TAG_STRING {
                    return Err(ValueError::Internal(
                        "Could not construct String object needed to create BigInt object".into(),
                    ));
                }

                let mut args = vec![*s];

                use crate::utils::js_null_value;

                let bigint_function = js_create_bigint_function(context);
                let bigint_function = OwnedValueRef::wrap(context, bigint_function);
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
        _ => unreachable!(),
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


/// A small wrapper around JSPropertyEnum, that frees resources that have to be freed
/// automatically when this goes out of scope.
pub struct OwnedPropertiesRef {
    value: *mut q::JSPropertyEnum,
    context: *mut q::JSContext,
    count: u32,
}

impl OwnedPropertiesRef {
    pub fn new(obj: &q::JSValue, context: *mut q::JSContext) -> Result<Self, ValueError> {
        let mut value: *mut q::JSPropertyEnum = std::ptr::null_mut();
        let mut count: u32 = 0;

        let flags = (q::JS_GPN_STRING_MASK | q::JS_GPN_SYMBOL_MASK | q::JS_GPN_ENUM_ONLY) as i32;
        let ret =
            unsafe { q::JS_GetOwnPropertyNames(context, &mut value, &mut count, *obj, flags) };
        if ret != 0 {
            return Err(ValueError::Internal(
                "Could not get object properties".into(),
            ));
        }

        Ok(Self { value, context, count })
    }
}

impl Drop for OwnedPropertiesRef {
    fn drop(&mut self) {
        let properties = &mut self.value;
        for index in 0..self.count {
            let prop = unsafe { properties.offset(index as isize) };
            unsafe {
                q::JS_FreeAtom(self.context, (*prop).atom);
            }
        }
        unsafe {
            q::js_free(self.context, self.value as *mut std::ffi::c_void);
        }
    }
}

impl std::ops::Deref for OwnedPropertiesRef {
    type Target = *mut q::JSPropertyEnum;

    fn deref(&self) -> &*mut q::JSPropertyEnum {
        &self.value
    }
}

impl std::ops::DerefMut for OwnedPropertiesRef {
    fn deref_mut(&mut self) -> &mut *mut q::JSPropertyEnum {
        &mut self.value
    }
}

fn deserialize_object(context: *mut q::JSContext, obj: &q::JSValue) -> Result<JsValue, ValueError> {
    assert_eq!(obj.tag, TAG_OBJECT);

    if unsafe { q::JS_IsFunction(context, *obj) } > 0 {
        return Ok(JsValue::OpaqueFunction(OwnedValueRef::owned(
            context, *obj,
        )));
    }

    let properties = OwnedPropertiesRef::new(obj, context)?;

    let mut map = HashMap::new();
    for index in 0..properties.count {
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

pub fn deserialize_value(
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
            let ptr = unsafe {
                q::JS_ToCStringLen2(context, std::ptr::null::<usize>() as *mut usize, *r, 0)
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
