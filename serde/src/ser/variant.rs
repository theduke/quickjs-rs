use std::ffi::CString;

use libquickjs_sys::{JSValue, JS_FreeValue, JS_NewObject, JS_SetPropertyStr};
use serde::Serialize;

use crate::context::Context;
use crate::errors::{Internal, SerializationError};
use crate::ser::map::SerializeMap;

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
        // we now need to create an object with the variant name with the value
        // as the inner object

        // IMPORTANT: we do this conversion before we call finish_object, that way if it
        // fails we don't have to worry about freeing the object
        let variant = CString::new(self.variant).map_err(Internal::from)?;

        let inner = self.inner.finish_object()?;

        let object = unsafe { JS_NewObject(self.inner.context.as_mut_ptr()) };
        // TODO: check in other places as well
        let object = SerializationError::try_from_value(self.inner.context.as_mut_ptr(), object)?;

        let result = unsafe {
            JS_SetPropertyStr(
                self.inner.context.as_mut_ptr(),
                object,
                variant.as_ptr(),
                inner,
            )
        };

        if result < 0 {
            unsafe { JS_FreeValue(self.inner.context.as_mut_ptr(), object) };
            unsafe { JS_FreeValue(self.inner.context.as_mut_ptr(), inner) };

            return Err(SerializationError::from_exception(
                self.inner.context.as_mut_ptr(),
            ));
        }

        Ok(object)
    }
}
