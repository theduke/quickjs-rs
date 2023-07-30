use libquickjs_sys::{
    JSContext, JSValue, JS_IsException, JS_NewBigInt64, JS_NewBigUint64, JS_NewBool, JS_NewFloat64,
    JS_NewInt32,
};
use serde::Serialize;

pub struct Serializer<'a> {
    context: &'a mut JSContext,
}

impl<'a> Serializer<'a> {
    pub fn new(context: &'a mut JSContext) -> Self {
        Self { context }
    }
}

impl<'a> serde::Serializer for Serializer<'a> {
    type Error = ();
    type Ok = JSValue;
    type SerializeMap = ();
    type SerializeSeq = ();
    type SerializeStruct = ();
    type SerializeStructVariant = ();
    type SerializeTuple = ();
    type SerializeTupleStruct = ();
    type SerializeTupleVariant = ();

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewBool(self.context, value) };

        Ok(value)
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewInt32(self.context, value) };

        Ok(value)
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        // try to fit the value into a 32-bit integer, otherwise return a BigInt
        if let Ok(value) = i32::try_from(value) {
            return self.serialize_i32(value);
        }

        let value = unsafe { JS_NewBigInt64(self.context, value) };

        Ok(value)
    }

    // For now we don't support i128 and u128, as there are no methods to create
    // BigInts for them.
    // In theory we could create our own function to do so, but for now that's
    // overkill.

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        // we cannot use `JS_NewInt32` here, as there are values in u32 that cannot be
        // represented in i32 (and would wrap around)
        self.serialize_u64(u64::from(value))
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        // try to fit the value into a 32-bit integer, otherwise return a BigInt
        // we could also call `serialize_u64` instead, but that is largely redundant.
        if let Ok(value) = i32::try_from(value) {
            return self.serialize_i32(value);
        }

        let value = unsafe { JS_NewBigUint64(self.context, value) };

        Ok(value)
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewFloat64(self.context, f64::from(value)) };

        if JS_IsException(value) {
            return Err(());
        }

        Ok(value)
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        todo!()
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        todo!()
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        todo!()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        todo!()
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        todo!()
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        todo!()
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        todo!()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        todo!()
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        todo!()
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        todo!()
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}
