mod map;
mod seq;
mod variant;

use std::ffi::CString;

use libquickjs_sys::{
    size_t, JSValue, JSValueUnion, JS_NewArrayBufferCopy, JS_NewBigInt64, JS_NewBigUint64,
    JS_NewBool, JS_NewFloat64, JS_NewInt32, JS_NewStringLen, JS_TAG_NULL,
};
use serde::ser::SerializeMap as _;
use serde::Serialize;

use crate::context::Context;
use crate::errors::{Internal, SerializationError};
use crate::ser::map::SerializeMap;
use crate::ser::seq::SerializeSeq;
use crate::ser::variant::{SerializeStructVariant, SerializeTupleVariant};

pub struct Serializer<'a> {
    context: &'a mut Context,
}

impl<'a> Serializer<'a> {
    pub fn new(context: &'a mut Context) -> Self {
        Self { context }
    }
}

impl<'a> serde::Serializer for Serializer<'a> {
    type Error = SerializationError;
    type Ok = JSValue;
    type SerializeMap = SerializeMap<'a>;
    type SerializeSeq = SerializeSeq<'a>;
    type SerializeStruct = SerializeMap<'a>;
    type SerializeStructVariant = SerializeStructVariant<'a>;
    type SerializeTuple = SerializeSeq<'a>;
    type SerializeTupleStruct = SerializeSeq<'a>;
    type SerializeTupleVariant = SerializeTupleVariant<'a>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewBool(self.context.as_mut_ptr(), value) };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(i32::from(value))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewInt32(self.context.as_mut_ptr(), value) };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        // try to fit the value into a 32-bit integer, otherwise return a BigInt
        if let Ok(value) = i32::try_from(value) {
            return self.serialize_i32(value);
        }

        let value = unsafe { JS_NewBigInt64(self.context.as_mut_ptr(), value) };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        if let Ok(value) = i64::try_from(value) {
            return self.serialize_i64(value);
        }

        return Err(SerializationError::IntTooLarge);
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

        let value = unsafe { JS_NewBigUint64(self.context.as_mut_ptr(), value) };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        if let Ok(value) = u64::try_from(value) {
            return self.serialize_u64(value);
        }

        Err(SerializationError::IntTooLarge)
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(value))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        let value = unsafe { JS_NewFloat64(self.context.as_mut_ptr(), value) };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        let mut buffer = [0; 4];
        let string = value.encode_utf8(&mut buffer);

        self.serialize_str(string)
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        let c_str = CString::new(value).map_err(Internal::from)?;

        let value = unsafe {
            JS_NewStringLen(
                self.context.as_mut_ptr(),
                c_str.as_ptr(),
                value.len() as size_t,
            )
        };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        // TODO: in theory we could also use `JS_NewArrayBuffer` here, but that would be
        // _a lot_ more complicated.
        let length = value.len();

        let value = unsafe {
            JS_NewArrayBufferCopy(self.context.as_mut_ptr(), value.as_ptr(), length as size_t)
        };
        SerializationError::try_from_value(self.context.as_mut_ptr(), value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        // Unit corresponds to `null` in JS

        // Taken from: https://docs.rs/quickjs_runtime/latest/src/quickjs_runtime/quickjs_utils/mod.rs.html#46-51
        let null = JSValue {
            u: JSValueUnion { int32: 0 },
            tag: i64::from(JS_TAG_NULL),
        };

        Ok(null)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        // We follow the same approach as serde_json here, and serialize the variant as
        // a string.
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        T::serialize(value, self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        // We follow the same approach as serde_json here, and serialize the variant as,
        // we serialize the value as an object with a single field.
        // { `variant`: `value` }

        let mut serializer = self.serialize_map(Some(1))?;
        serializer.serialize_entry(variant, value)?;
        serializer.end()
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        SerializeSeq::new(self.context)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        SerializeSeq::new(self.context)
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        SerializeSeq::new(self.context)
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        SerializeTupleVariant::new(variant, self.context)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        SerializeMap::new(self.context)
    }

    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        SerializeMap::new(self.context)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        SerializeStructVariant::new(variant, self.context)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}
