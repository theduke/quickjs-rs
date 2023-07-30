use libquickjs_sys::{
    JSAtom, JSContext, JSValue, JS_FreeAtom, JS_FreeValue, JS_GetException, JS_IsNull,
    JS_NewObject, JS_SetProperty, JS_ValueToAtom, JS_ATOM_NULL,
};
use serde::Serialize;

use crate::context::Context;
use crate::errors::SerializationError;
use crate::ser::Serializer;

pub struct SerializeMap<'a> {
    context: &'a mut Context,
    object: Option<JSValue>,

    pending_key: Option<JSValue>,
    buffer: Vec<(JSValue, JSValue)>,
}

impl<'a> SerializeMap<'a> {
    pub(crate) fn new(context: &'a mut Context) -> Self {
        let object = unsafe { JS_NewObject(context.as_mut_ptr()) };

        Self {
            context,
            object: Some(object),
        }
    }

    fn key_to_atom(&mut self, key: JSValue) -> Result<JSAtom, SerializationError> {
        let atom = unsafe { JS_ValueToAtom(self.context.as_mut_ptr(), key) };

        // free the key value
        unsafe { JS_FreeValue(self.context.as_mut_ptr(), key) };

        if atom == JS_ATOM_NULL {
            return Err(SerializationError::InvalidKey);
        }

        Ok(atom)
    }

    fn insert(&mut self, key: JSValue, value: JSValue) -> Result<(), SerializationError> {
        let object = self.object.expect("object is not initialized");

        let key = self.key_to_atom(key)?;

        let error = unsafe { JS_SetProperty(self.context.as_mut_ptr(), object, key, value) };

        if error == -1 {
            // exception occurred, time to roll back
            let error = SerializationError::from_exception(self.context.as_mut_ptr());

            // free the value and key
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), value) };
            unsafe { JS_FreeAtom(self.context.as_mut_ptr(), key) };

            return Err(error);
        }

        // The value is freed by JS_SetProperty, the key is not freed
        unsafe { JS_FreeAtom(self.context.as_mut_ptr(), key) };

        Ok(())
    }
}

impl<'a> serde::ser::SerializeMap for SerializeMap<'a> {
    type Error = SerializationError;
    type Ok = JSValue;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let serializer = Serializer::new(self.context);
        let value = key.serialize(serializer)?;

        self.pending_key = Some(value);
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key = self
            .pending_key
            .take()
            .ok_or(SerializationError::MissingKey)?;

        let serializer = Serializer::new(self.context);
        let value = value.serialize(serializer)?;

        self.buffer.push((key, value));
        Ok(())
    }

    fn serialize_entry<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error>
    where
        K: Serialize,
        V: Serialize,
    {
        // we don't need to buffer the key, we can serialize it directly

        let key = {
            let serializer = Serializer::new(self.context);
            key.serialize(serializer)?
        };

        let value = {
            let serializer = Serializer::new(self.context);
            value.serialize(serializer)?
        };

        self.insert(key, value)
    }

    // TODO: when does Drop get called? (I hope after this function)
    fn end(mut self) -> Result<Self::Ok, Self::Error> {
        if self.pending_key.is_some() {
            return Err(SerializationError::MissingValue);
        }

        // insert the buffered values
        for (key, value) in self.buffer.drain(..) {
            self.insert(key, value)?;
        }

        let object = self.object.take().expect("object is not initialized");
        Ok(object)
    }
}

impl<'a> serde::ser::SerializeStruct for SerializeMap<'a> {
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
        <Self as serde::ser::SerializeMap>::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        <Self as serde::ser::SerializeMap>::end(self)
    }
}

impl Drop for SerializeMap<'_> {
    fn drop(&mut self) {
        // free the object
        if let Some(object) = self.object.take() {
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), object) };
        }

        // free the pending key
        if let Some(key) = self.pending_key.take() {
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), key) };
        }

        // free the buffer
        for (key, value) in self.buffer.drain(..) {
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), key) };
            unsafe { JS_FreeValue(self.context.as_mut_ptr(), value) };
        }
    }
}
