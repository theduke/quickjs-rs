use std::ffi::CString;

use libquickjs_sys::{JSContext, JSValue, JS_FreeValue, JS_NewObject, JS_SetPropertyStr};
use serde::Serialize;

use crate::context::Context;
use crate::errors::{Internal, SerializationError};
use crate::ser::map::SerializeMap;
use crate::ser::seq::SerializeSeq;

/// Serialize an enum variant.
///
/// Serializes an enum variant as `{variant: value}`.
fn finish(
    context: *mut JSContext,
    variant: &'static str,
    value: JSValue,
) -> Result<JSValue, SerializationError> {
    // IMPORTANT: we do this conversion before we call finish_object, that way if it
    // fails we don't have to worry about freeing the object
    // The only one we need to worry about is the given value.
    let variant = CString::new(variant).map_err(Internal::from);

    let variant = match variant {
        Ok(variant) => variant,
        Err(error) => {
            // ensure that we don't memory leak
            unsafe { JS_FreeValue(context, value) };
            return Err(SerializationError::Internal(error));
        }
    };

    let object = unsafe { JS_NewObject(context) };
    // TODO: check in other places as well
    let object = SerializationError::try_from_value(context, object)?;

    let result = unsafe { JS_SetPropertyStr(context, object, variant.as_ptr(), value) };

    if result < 0 {
        unsafe { JS_FreeValue(context, object) };
        unsafe { JS_FreeValue(context, value) };

        return Err(SerializationError::from_exception(context));
    }

    Ok(object)
}

pub struct SerializeStructVariant<'a> {
    variant: &'static str,

    inner: SerializeMap<'a>,
}

impl<'a> SerializeStructVariant<'a> {
    pub fn new(
        variant: &'static str,
        context: &'a mut Context,
    ) -> Result<Self, SerializationError> {
        let inner = SerializeMap::new(context)?;

        Ok(Self { variant, inner })
    }
}

impl<'a> serde::ser::SerializeStructVariant for SerializeStructVariant<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        <SerializeMap<'a> as serde::ser::SerializeMap>::serialize_entry(&mut self.inner, key, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        let inner = self.inner.finish_object()?;

        finish(self.inner.context.as_mut_ptr(), self.variant, inner)
    }
}

pub struct SerializeTupleVariant<'a> {
    variant: &'static str,

    inner: SerializeSeq<'a>,
}

impl<'a> SerializeTupleVariant<'a> {
    pub fn new(
        variant: &'static str,
        context: &'a mut Context,
    ) -> Result<Self, SerializationError> {
        let inner = SerializeSeq::new(context)?;

        Ok(Self { variant, inner })
    }
}

impl<'a> serde::ser::SerializeTupleVariant for SerializeTupleVariant<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        <SerializeSeq<'a> as serde::ser::SerializeSeq>::serialize_element(&mut self.inner, value)
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        let inner = self.inner.finish_array()?;

        finish(self.inner.context.as_mut_ptr(), self.variant, inner)
    }
}
