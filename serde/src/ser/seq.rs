use libquickjs_sys::{JSValue, JS_FreeValue, JS_NewArray, JS_SetPropertyUint32};
use serde::Serialize;

use crate::context::Context;
use crate::errors::SerializationError;
use crate::ser::Serializer;

pub struct SerializeSeq<'a> {
    context: &'a mut Context,

    count: u32,
    array: Option<JSValue>,
}

impl<'a> SerializeSeq<'a> {
    pub fn new(context: &'a mut Context) -> Self {
        let array = unsafe { JS_NewArray(context.as_mut_ptr()) };

        Self {
            context,
            count: 0,
            array: Some(array),
        }
    }
}

impl<'a> serde::ser::SerializeSeq for SerializeSeq<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let value = {
            let serializer = Serializer::new(self.context);
            value.serialize(serializer)?
        };

        let array = self.array.expect("array is not initialized");
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

    // TODO: investigate Drop
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        Ok(self.array.take().expect("array is not initialized"))
    }
}

impl Drop for SerializeSeq<'_> {
    fn drop(&mut self) {
        if let Some(array) = self.array.take() {
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), array) };
        }
    }
}
