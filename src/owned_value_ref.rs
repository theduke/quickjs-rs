use crate::bindings::{TAG_INVALID, TAG_NULL, TAG_BOOL, TAG_INT, TAG_FLOAT64, TAG_EXCEPTION, TAG_OBJECT, TAG_STRING, TAG_UNDEFINED};
use crate::utils::{free_value, make_cstring};
use crate::{ExecutionError, JsValue, ValueError};
use libquickjs_sys as q;
use crate::marshal::deserialize_value;

/// OwnedValueRef wraps a Javascript value from the quickjs runtime.
/// It prevents leaks by ensuring that the inner value is deallocated on drop.
pub struct OwnedValueRef {
    pub(crate) context: *mut q::JSContext,
    pub(crate) value: q::JSValue,
}

// OwnedValueRef is NOT thread-safe, because q::JSValue is not.
// Send+Sync impl is required because quick_js::ContextError must be thread-safe and is allowed to carry a JsValue.
// A JsValue may carry an OwnedValueRef for wrapping a js-function.
// quick_js::ContextError will never print or access the OpaqueFunction enum option of JsValue though,
// therefore this is safe.
unsafe impl Send for OwnedValueRef {}

unsafe impl Sync for OwnedValueRef {}

impl Drop for OwnedValueRef {
    fn drop(&mut self) {
        unsafe {
            if self.value.tag != TAG_INVALID {
                free_value(self.context, self.value);
            }
        }
    }
}

impl Clone for OwnedValueRef {
    fn clone(&self) -> Self {
        // All tags < 0 are garbage collected and the reference count need to be adapted
        if self.value.tag < 0 {
            // This transmute is OK since if tag < 0, the union will be a refcount pointer.
            let ptr = unsafe { self.value.u.ptr as *mut q::JSRefCountHeader };
            let pref: &mut q::JSRefCountHeader = unsafe { &mut *ptr };
            pref.ref_count += 1;
        }
        return OwnedValueRef {
            context: self.context,
            value: self.value,
        };
    }
}

impl std::fmt::Debug for OwnedValueRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.value.tag {
            TAG_EXCEPTION => write!(f, "Exception(?)"),
            TAG_NULL => write!(f, "NULL"),
            TAG_UNDEFINED => write!(f, "UNDEFINED"),
            TAG_BOOL => write!(f, "Bool(?)", ),
            TAG_INT => write!(f, "Int(?)"),
            TAG_FLOAT64 => write!(f, "Float(?)"),
            TAG_STRING => write!(f, "String(?)"),
            TAG_OBJECT => write!(f, "Object(?)"),
            _ => write!(f, "?"),
        }
    }
}

impl PartialEq for OwnedValueRef {
    fn eq(&self, other: &Self) -> bool {
        self.value.tag == other.value.tag && unsafe { self.value.u.int32 == other.value.u.int32 }
    }
}

impl OwnedValueRef {
    /// Wrap a QuickJS JSValue in an owned reference.
    ///
    /// For reference counted JSValues:
    /// The reference count is not changed while wrapping.
    /// It will decrease however when dropping this wrapper (freeing the value if necessary).
    pub fn wrap(context: *mut q::JSContext, value: q::JSValue) -> Self {
        OwnedValueRef {
            context,
            value,
        }
    }
    /// Increases the reference count of the given value (if any) and therefore fully
    /// own the value.
    pub fn owned(context: *mut q::JSContext, value: q::JSValue) -> Self {
        // All tags < 0 are garbage collected and need to be freed.
        if value.tag < 0 {
            // This transmute is OK since if tag < 0, the union will be a refcount
            // pointer.
            let ptr = unsafe { value.u.ptr as *mut q::JSRefCountHeader };
            let pref: &mut q::JSRefCountHeader = unsafe { &mut *ptr };
            pref.ref_count += 1;
        }
        OwnedValueRef {
            context,
            value,
        }
    }

    /// Get the inner JSValue without freeing in drop.
    ///
    /// Unsafe because the caller is responsible for freeing the value.
    #[allow(dead_code)]
    unsafe fn into_inner(mut self) -> q::JSValue {
        let v = self.value;
        self.value = q::JSValue {
            u: q::JSValueUnion { int32: 0 },
            tag: TAG_INVALID,
        };
        v
    }

    /// Return the reference count value. Useful for debugging
    #[allow(dead_code)]
    pub fn ref_count(&self) -> i32 {
        if self.value.tag < 0 {
            // This transmute is OK since if tag < 0, the union will be a refcount
            // pointer.
            let ptr = unsafe { self.value.u.ptr as *mut q::JSRefCountHeader };
            let pref: &mut q::JSRefCountHeader = unsafe { &mut *ptr };
            pref.ref_count
        } else {
            0
        }
    }

    /// Return true if this is a null value
    pub fn is_null(&self) -> bool {
        self.value.tag == TAG_NULL
    }

    /// Return true if this is a boolean value
    pub fn is_bool(&self) -> bool {
        self.value.tag == TAG_BOOL
    }

    /// Return true if this is an exception
    pub fn is_exception(&self) -> bool {
        self.value.tag == TAG_EXCEPTION
    }

    /// Return true if this is an object
    pub fn is_object(&self) -> bool {
        self.value.tag == TAG_OBJECT
    }

    /// Return true if this is a string value
    pub fn is_string(&self) -> bool {
        self.value.tag == TAG_STRING
    }

    /// Return the string value or convert the value to a string and return it
    pub fn to_string(&self) -> Result<String, ExecutionError> {
        let value = if self.is_string() {
            self.to_value()?
        } else {
            let raw = unsafe { q::JS_ToString(self.context, self.value) };
            let value = OwnedValueRef::wrap(self.context, raw);

            if value.value.tag != TAG_STRING {
                return Err(ExecutionError::Exception(
                    "Could not convert value to string".into(),
                ));
            }
            value.to_value()?
        };

        Ok(value.as_str().unwrap().to_string())
    }

    /// Deserialize the inner QuickJS JSValue into a Rust type JsValue
    pub fn to_value(&self) -> Result<JsValue, ValueError> {
        deserialize_value(self.context, &self.value)
    }

    /// Return the boolean value or an error
    pub fn to_bool(&self) -> Result<bool, ValueError> {
        match self.to_value()? {
            JsValue::Bool(b) => Ok(b),
            _ => Err(ValueError::UnexpectedType),
        }
    }
}

/// Wraps an object from the quickjs runtime.
/// Provides convenience property accessors.
pub struct OwnedObjectRef {
    value: OwnedValueRef,
}

impl OwnedObjectRef {
    pub fn new(value: OwnedValueRef) -> Result<Self, ValueError> {
        if value.value.tag != TAG_OBJECT {
            Err(ValueError::Internal("Expected an object".into()))
        } else {
            Ok(Self { value })
        }
    }

    pub(crate) fn into_value(self) -> OwnedValueRef {
        self.value
    }

    /// Get the tag of a property.
    fn property_tag(&self, name: &str) -> Result<i64, ValueError> {
        let cname = make_cstring(name)?;
        let raw = unsafe {
            q::JS_GetPropertyStr(self.value.context, self.value.value, cname.as_ptr())
        };
        let t = raw.tag;
        unsafe {
            free_value(self.value.context, raw);
        }
        Ok(t)
    }

    /// Determine if the object is a promise by checking the presence of
    /// a 'then' and a 'catch' property.
    pub(crate) fn is_promise(&self) -> Result<bool, ValueError> {
        if self.property_tag("then")? == TAG_OBJECT && self.property_tag("catch")? == TAG_OBJECT {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn property(&self, name: &str) -> Result<OwnedValueRef, ExecutionError> {
        let cname = make_cstring(name)?;
        let raw = unsafe {
            q::JS_GetPropertyStr(self.value.context, self.value.value, cname.as_ptr())
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
            Ok(OwnedValueRef::wrap(self.value.context, raw))
        }
    }

    pub(crate) unsafe fn set_property_raw(
        &self,
        name: &str,
        value: q::JSValue,
    ) -> Result<(), ExecutionError> {
        let cname = make_cstring(name)?;
        let ret = q::JS_SetPropertyStr(
            self.value.context,
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
