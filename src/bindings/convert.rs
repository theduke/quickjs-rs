use std::{collections::HashMap, os::raw::c_char};

use libquickjs_sys as q;

use crate::{JsValue, ValueError};

use super::{droppable_value::DroppableValue, make_cstring};

use super::{
    TAG_BOOL, TAG_EXCEPTION, TAG_FLOAT64, TAG_INT, TAG_NULL, TAG_OBJECT, TAG_STRING, TAG_UNDEFINED,
};

#[cfg(feature = "bigint")]
use {
    super::TAG_BIG_INT,
    crate::value::bigint::{BigInt, BigIntOrI64},
};

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
    unsafe { q::JS_FreeValue(context, global) };
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
    unsafe { q::JS_FreeValue(context, global) };
    bigint_function
}

/// Serialize a Rust value into a quickjs runtime value.
pub(super) fn serialize_value(
    context: *mut q::JSContext,
    value: JsValue,
) -> Result<q::JSValue, ValueError> {
    let v = match value {
        JsValue::Undefined => q::JSValue {
            u: q::JSValueUnion { int32: 0 },
            tag: TAG_UNDEFINED,
        },
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
                            q::JS_FreeValue(context, arr);
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
                        q::JS_FreeValue(context, arr);
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
                        q::JS_FreeValue(context, obj);
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
                        q::JS_FreeValue(context, obj);
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
                q::JS_FreeValue(context, date_constructor);
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
                        bigint_string.len() as q::size_t,
                    )
                };
                let s = DroppableValue::new(s, |&mut s| unsafe {
                    q::JS_FreeValue(context, s);
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
                        q::JS_FreeValue(context, bigint_function);
                    });
                let js_bigint = unsafe {
                    q::JS_Call(
                        context,
                        *bigint_function,
                        q::JSValue {
                            u: q::JSValueUnion { int32: 0 },
                            tag: TAG_NULL,
                        },
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
    unsafe { q::JS_FreeValue(context, len_raw) };
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
        unsafe { q::JS_FreeValue(context, value_raw) };

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
            q::JS_FreeValue(context, raw_value);
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
            q::JS_FreeValue(context, key_value);
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

pub(super) fn deserialize_value(
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
        TAG_UNDEFINED => Ok(JsValue::Undefined),
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
                            q::JS_FreeValue(context, getter);
                            q::JS_FreeValue(context, date_constructor);
                        };

                        let res = if timestamp_raw.tag == TAG_FLOAT64 {
                            let f = unsafe { timestamp_raw.u.float64 } as i64;
                            let datetime = chrono::Utc.timestamp_millis(f);
                            Ok(JsValue::Date(datetime))
                        } else if timestamp_raw.tag == TAG_INT {
                            let f = unsafe { timestamp_raw.u.int32 } as i64;
                            let datetime = chrono::Utc.timestamp_millis(f);
                            Ok(JsValue::Date(datetime))
                        } else {
                            Err(ValueError::Internal(
                                "Could not convert 'Date' instance to timestamp".into(),
                            ))
                        };
                        return res;
                    } else {
                        unsafe { q::JS_FreeValue(context, date_constructor) };
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
