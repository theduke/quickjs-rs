use libquickjs_sys::{JSValue, JS_FreeValue, JS_NewArray, JS_SetPropertyUint32};
use serde::Serialize;

use crate::context::Context;
use crate::errors::SerializationError;
use crate::ser::Serializer;

pub struct SerializeSeq<'a> {
    pub(crate) context: &'a mut Context,

    count: u32,
    array: Option<JSValue>,
}

impl<'a> SerializeSeq<'a> {
    pub fn new(context: &'a mut Context) -> Result<Self, SerializationError> {
        let array = unsafe { JS_NewArray(context.as_mut_ptr()) };
        let array = SerializationError::try_from_value(context.as_mut_ptr(), array)
            .expect("failed to create array");

        Ok(Self {
            context,
            count: 0,
            array: Some(array),
        })
    }

    pub(crate) fn finish_array(&mut self) -> Result<JSValue, SerializationError> {
        self.array.take().ok_or(SerializationError::InvalidState)
    }
}

impl<'a> serde::ser::SerializeSeq for SerializeSeq<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        // IMPORTANT: This is on top, so that we don't need to free the value in case of
        // an error.
        let array = self.array.ok_or(SerializationError::InvalidState)?;

        let value = {
            let serializer = Serializer::new(self.context);
            value.serialize(serializer)?
        };

        let error =
            unsafe { JS_SetPropertyUint32(self.context.as_mut_ptr(), array, self.count, value) };

        if error == -1 {
            // exception occurred, time to roll back
            let error = SerializationError::from_exception(self.context.as_mut_ptr());

            // free the value
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), value) };

            return Err(error);
        }

        self.count += 1;
        Ok(())
    }

    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        self.finish_array()
    }
}

impl<'a> serde::ser::SerializeTuple for SerializeSeq<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        <Self as serde::ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as serde::ser::SerializeSeq>::end(self)
    }
}

impl<'a> serde::ser::SerializeTupleStruct for SerializeSeq<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        <Self as serde::ser::SerializeSeq>::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as serde::ser::SerializeSeq>::end(self)
    }
}

impl Drop for SerializeSeq<'_> {
    fn drop(&mut self) {
        if let Some(array) = self.array.take() {
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), array) };
        }
    }
}
