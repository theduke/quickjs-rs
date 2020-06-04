// imports {{{
use std;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::ops::{AddAssign, MulAssign, Neg};

use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::Deserialize;
use serde::{ser, Serialize};

use super::JsValue;
// }}}

// error {{{
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    Message(String),
    Eof,
    Syntax,
    ExpectedBoolean,
    ExpectedInteger,
    ExpectedString,
    ExpectedNull,
    ExpectedArray,
    ExpectedArrayComma,
    ExpectedArrayEnd,
    ExpectedMap,
    ExpectedMapColon,
    ExpectedMapComma,
    ExpectedMapEnd,
    ExpectedEnum,
    TrailingCharacters,
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            _ => unimplemented!(),
        }
    }
}

impl std::error::Error for Error {}
// }}}

// ser {{{
pub struct Serializer {
    pending_hashmap_key: Option<String>,
    pending_js_value: Vec<JsValue>,
}

// impl Serializer {{{
impl Serializer {
    fn serialize_keyvalue_to_pending_map<K: ?Sized, V: ?Sized>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
    {
        let key = key.serialize(&mut *self)?;
        let key_as_string = match key {
            JsValue::String(s) => s,
            _ => return Err(Error::Message("Expected key to be a string".into())),
        };
        let value = value.serialize(&mut *self)?;
        match &mut self.pending_js_value.last_mut() {
            Some(JsValue::Object(map)) => {
                map.insert(key_as_string, value);
                Ok(())
            }
            _ => {
                return Err(Error::Message(
                    "Inner pending object is not a JsValue::Object".into(),
                ))
            }
        }
    }

    fn end_pending_map(&mut self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending object is None".into()))
    }

    fn serialize_element_to_pending_array<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let value = value.serialize(&mut *self)?;
        match &mut self.pending_js_value.last_mut() {
            Some(JsValue::Array(arr)) => {
                arr.push(value);
                Ok(())
            }
            _ => return Err(Error::Message("Inner pending value is not an array".into())),
        }
    }

    fn end_pending_array(&mut self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending value is None".into()))
    }

    fn wrap_in_map_with_key(&self, key: String, value: JsValue) -> JsValue {
        let mut map = HashMap::new();
        map.insert(key, value);
        JsValue::Object(map)
    }
}
// }}}

pub fn to_js_value<T>(value: &T) -> Result<JsValue>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        pending_hashmap_key: None,
        pending_js_value: vec![],
    };
    value.serialize(&mut serializer)
}

// impl ser::Serializer {{{
impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        Ok(JsValue::Bool(v))
    }

    // JSON does not distinguish between different sizes of integers, so all
    // signed integers will be serialized the same and all unsigned integers
    // will be serialized the same. Other formats, especially compact binary
    // formats, may need independent logic for the different sizes.
    fn serialize_i8(self, v: i8) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_i16(self, v: i16) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_i32(self, v: i32) -> Result<JsValue> {
        Ok(JsValue::Int(v))
    }

    // Not particularly efficient but this is example code anyway. A more
    // performant approach would be to use the `itoa` crate.
    fn serialize_i64(self, v: i64) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_u8(self, v: u8) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_u16(self, v: u16) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_u32(self, v: u32) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_u64(self, v: u64) -> Result<JsValue> {
        Ok(JsValue::Int(v as i32))
    }

    fn serialize_f32(self, v: f32) -> Result<JsValue> {
        Ok(JsValue::Float(v.into()))
    }

    fn serialize_f64(self, v: f64) -> Result<JsValue> {
        Ok(JsValue::Float(f64::from(v)))
    }

    fn serialize_char(self, v: char) -> Result<JsValue> {
        Ok(JsValue::String(format!("{}", v)))
    }

    fn serialize_str(self, v: &str) -> Result<JsValue> {
        Ok(JsValue::String(v.into()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<JsValue> {
        let mut vec = Vec::<JsValue>::with_capacity(v.len());
        for i in v {
            vec.push(self.serialize_u8(*i)?);
        }
        Ok(JsValue::Array(vec))
    }

    fn serialize_none(self) -> Result<JsValue> {
        Ok(JsValue::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<JsValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<JsValue> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<JsValue> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<JsValue> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<JsValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    /// Serialize a newtype variant like E::N in enum E { N(u8) }
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<JsValue>
    where
        T: ?Sized + Serialize,
    {
        let mut map = HashMap::new();
        map.insert(variant.into(), value.serialize(self)?);
        Ok(JsValue::Object(map))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.pending_js_value.push(JsValue::Array(vec![]));
        Ok(self)
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently by omitting the length, since tuple
    // means that the corresponding `Deserialize implementation will know the
    // length without needing to look at the serialized data.
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    // Tuple structs look just like sequences in JSON.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }`. Again
    // this method is only responsible for the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.pending_hashmap_key = Some(variant.into());
        self.pending_js_value.push(JsValue::Array(vec![]));
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        self.pending_js_value.push(JsValue::Object(HashMap::new()));
        Ok(self)
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.pending_hashmap_key = Some(variant.into());
        self.pending_js_value.push(JsValue::Object(HashMap::new()));
        Ok(self)
    }
}
// }}}

// impl ser::SerializeSeq {{{
impl<'a> ser::SerializeSeq for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_element_to_pending_array(value)
    }

    fn end(self) -> Result<JsValue> {
        self.end_pending_array()
    }
}
// }}}

// ser::SerializeTuple {{{
impl<'a> ser::SerializeTuple for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_element_to_pending_array(value)
    }

    fn end(self) -> Result<JsValue> {
        self.end_pending_array()
    }
}
// }}}

// ser::SerializeTupleStruct {{{
impl<'a> ser::SerializeTupleStruct for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // if !self.output.ends_with('[') {
        //     self.output += ",";
        // }
        // value.serialize(&mut **self)
        todo!()
    }

    fn end(self) -> Result<JsValue> {
        // self.output += "]";
        // Ok(())
        todo!()
    }
}
// }}}

// ser::SerializeTupleVariant {{{
// Tuple variants are a little different. Refer back to the
// `serialize_tuple_variant` method above:
//
//    self.output += "{";
//    variant.serialize(&mut *self)?;
//    self.output += ":[";
//
// So the `end` method in this impl is responsible for closing both the `]` and
// the `}`.
impl<'a> ser::SerializeTupleVariant for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_element_to_pending_array(value)
    }

    fn end(self) -> Result<JsValue> {
        let value = self.end_pending_array()?;
        let key = self
            .pending_hashmap_key
            .take()
            .ok_or(Error::Message("Inner pending HashMap key is None".into()))?;
        Ok(self.wrap_in_map_with_key(key, value))
    }
}
// }}}

// ser::SerializeMap {{{
// Some `Serialize` types are not able to hold a key and value in memory at the
// same time so `SerializeMap` implementations are required to support
// `serialize_key` and `serialize_value` individually.
//
// There is a third optional method on the `SerializeMap` trait. The
// `serialize_entry` method allows serializers to optimize for the case where
// key and value are both available simultaneously. In JSON it doesn't make a
// difference so the default behavior for `serialize_entry` is fine.
impl<'a> ser::SerializeMap for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    // empty methods (unneeded) {{{
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Ok(())
    }
    // }}}

    fn serialize_entry<K: ?Sized, V: ?Sized>(&mut self, key: &K, value: &V) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
    {
        self.serialize_keyvalue_to_pending_map(key, value)
    }

    fn end(self) -> Result<JsValue> {
        self.end_pending_map()
    }
}
// }}}

// ser::SerializeStruct {{{
// Structs are like maps in which the keys are constrained to be compile-time
// constant strings.
impl<'a> ser::SerializeStruct for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_keyvalue_to_pending_map(key, value)
    }

    fn end(self) -> Result<JsValue> {
        self.end_pending_map()
    }
}
// }}}

// ser::SerializeStructVariant {{{
// Similar to `SerializeTupleVariant`, here the `end` method is responsible for
// closing both of the curly braces opened by `serialize_struct_variant`.
impl<'a> ser::SerializeStructVariant for &'a mut Serializer {
    type Ok = JsValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_keyvalue_to_pending_map(key, value)
    }

    fn end(self) -> Result<JsValue> {
        let key = self
            .pending_hashmap_key
            .take()
            .ok_or(Error::Message("The pending HashMap key is None".into()))?;
        let value = self.end_pending_map()?;
        Ok(self.wrap_in_map_with_key(key, value))
    }
}
// }}}

// ser tests {{{
#[test]
fn test_ser_struct() {
    use serde::Serialize;

    #[derive(Serialize)]
    struct Test {
        int: u32,
        seq: Vec<&'static str>,
    }

    let test = Test {
        int: 1,
        seq: vec!["a", "b"],
    };
    let mut map = HashMap::new();
    map.insert("int".into(), JsValue::Int(1));
    map.insert(
        "seq".into(),
        JsValue::Array(vec![
            JsValue::String("a".into()),
            JsValue::String("b".into()),
        ]),
    );
    let expected = JsValue::Object(map);
    assert_eq!(to_js_value(&test).unwrap(), expected);

    let tuple = (1, 2);
    let expected = JsValue::Array(vec![JsValue::Int(1), JsValue::Int(2)]);
    assert_eq!(to_js_value(&tuple).unwrap(), expected);
}

#[test]
fn test_ser_enum() {
    #[derive(Serialize)]
    enum E {
        Unit,
        Newtype(u32),
        Tuple(u32, u32),
        Struct { a: u32 },
    }

    let u = E::Unit;
    let expected = JsValue::String("Unit".into());
    assert_eq!(to_js_value(&u).unwrap(), expected);

    let n = E::Newtype(1);
    let mut map = HashMap::new();
    map.insert("Newtype".into(), JsValue::Int(1));
    let expected = JsValue::Object(map);
    assert_eq!(to_js_value(&n).unwrap(), expected);

    let t = E::Tuple(1, 2);
    let mut map = HashMap::new();
    map.insert(
        "Tuple".into(),
        JsValue::Array(vec![JsValue::Int(1), JsValue::Int(2)]),
    );
    let expected = JsValue::Object(map);
    assert_eq!(to_js_value(&t).unwrap(), expected);

    let s = E::Struct { a: 1 };
    let mut inner_map = HashMap::new();
    inner_map.insert("a".into(), JsValue::Int(1));
    let mut map = HashMap::new();
    map.insert("Struct".into(), JsValue::Object(inner_map));
    let expected = JsValue::Object(map);
    assert_eq!(to_js_value(&s).unwrap(), expected);
}
// }}}
// }}}

// de {{{
pub struct Deserializer<'de> {
    pending_js_value: Vec<&'de JsValue>,
}

impl<'de> Deserializer<'de> {
    pub fn from_js_value(input: &'de JsValue) -> Self {
        Deserializer {
            pending_js_value: vec![input],
        }
    }
}

pub fn from_js_value<'a, T>(val: &'a JsValue) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_js_value(val);
    T::deserialize(&mut deserializer)
}

// de::Deserializer {{{
impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Object(map)) => self.deserialize_map(visitor),
            Some(JsValue::Array(v)) => self.deserialize_seq(visitor),
            Some(JsValue::String(s)) => self.deserialize_string(visitor),
            #[cfg(feature = "chrono")]
            Some(JsValue::Date(d)) => todo!(),
            #[cfg(feature = "num-bigint")]
            Some(JsValue::BigInt(bi)) => todo!(),
            Some(JsValue::Float(f)) => self.deserialize_f64(visitor),
            Some(JsValue::Int(i)) => self.deserialize_i64(visitor),
            Some(JsValue::Bool(b)) => self.deserialize_bool(visitor),
            Some(JsValue::Null) => self.deserialize_option(visitor),
            _ => return Err(Error::Message("Pending JS Value is invalid".into())),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Bool(b)) => visitor.visit_bool(*b),
            _ => Err(Error::Message("Expected bool".into())),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_i8(*i as i8),
            Some(JsValue::Float(f)) => visitor.visit_i8(*f as i8),
            _ => Err(Error::Message("Expected i8".into())),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_i16(*i as i16),
            Some(JsValue::Float(f)) => visitor.visit_i16(*f as i16),
            _ => Err(Error::Message("Expected i16".into())),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_i32(*i as i32),
            Some(JsValue::Float(f)) => visitor.visit_i32(*f as i32),
            _ => Err(Error::Message("Expected i32".into())),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_i64(*i as i64),
            Some(JsValue::Float(f)) => visitor.visit_i64(*f as i64),
            _ => Err(Error::Message("Expected i64".into())),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_u8(*i as u8),
            Some(JsValue::Float(f)) => visitor.visit_u8(*f as u8),
            _ => Err(Error::Message("Expected u8".into())),
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_u16(*i as u16),
            Some(JsValue::Float(f)) => visitor.visit_u16(*f as u16),
            _ => Err(Error::Message("Expected u16".into())),
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_u32(*i as u32),
            Some(JsValue::Float(f)) => visitor.visit_u32(*f as u32),
            _ => Err(Error::Message("Expected u32".into())),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_u64(*i as u64),
            Some(JsValue::Float(f)) => visitor.visit_u64(*f as u64),
            _ => Err(Error::Message("Expected u64".into())),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_f32(*i as f32),
            Some(JsValue::Float(f)) => visitor.visit_f32(*f as f32),
            _ => Err(Error::Message("Expected f32".into())),
        }
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Int(i)) => visitor.visit_f64(*i as f64),
            Some(JsValue::Float(f)) => visitor.visit_f64(*f as f64),
            _ => Err(Error::Message("Expected f64".into())),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::String(s)) if s.len() == 1 => {
                visitor.visit_char(s.chars().nth(0).unwrap() as char)
            }
            _ => Err(Error::Message("Expected char".into())),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::String(s)) => visitor.visit_str(s),
            _ => Err(Error::Message("Expected str".into())),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::String(s)) => visitor.visit_string(s.clone()),
            _ => Err(Error::Message("Expected String".into())),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Null) => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.pending_js_value.last() {
            Some(JsValue::Null) => visitor.visit_none(),
            _ => Err(Error::Message("Expected ()".into())),
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // Deserialization of compound types like sequences and maps happens by
    // passing the visitor an "Access" object that gives it the ability to
    // iterate through the data contained in the sequence.
    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(NestedAccess::new(&mut self))
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently.
    //
    // As indicated by the length parameter, the `Deserialize` implementation
    // for a tuple in the Serde data model is required to know the length of the
    // tuple before even looking at the input data.
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Tuple structs look just like sequences in JSON.
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Much like `deserialize_seq` but calls the visitors `visit_map` method
    // with a `MapAccess` implementation, rather than the visitor's `visit_seq`
    // method with a `SeqAccess` implementation.
    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(NestedAccess::new(&mut *self))
    }

    // Structs look just like maps in JSON.
    //
    // Notice the `fields` parameter - a "struct" in the Serde data model means
    // that the `Deserialize` implementation is required to know what the fields
    // are before even looking at the input data. Any key-value pairing in which
    // the fields cannot be known ahead of time is probably a map.
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // if self.peek_char()? == '"' {
        //     // Visit a unit variant.
        //     visitor.visit_enum(self.parse_string()?.into_deserializer())
        // } else if self.next_char()? == '{' {
        //     // Visit a newtype variant, tuple variant, or struct variant.
        //     let value = visitor.visit_enum(Enum::new(self))?;
        //     // Parse the matching close brace.
        //     if self.next_char()? == '}' {
        //         Ok(value)
        //     } else {
        //         Err(Error::ExpectedMapEnd)
        //     }
        // } else {
        //     Err(Error::ExpectedEnum)
        // }
        todo!()
    }

    // An identifier in Serde is the type that identifies a field of a struct or
    // the variant of an enum. In JSON, struct fields and enum variants are
    // represented as strings. In other formats they may be represented as
    // numeric indices.
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // Like `deserialize_any` but indicates to the `Deserializer` that it makes
    // no difference which `Visitor` method is called because the data is
    // ignored.
    //
    // Some deserializers are able to implement this more efficiently than
    // `deserialize_any`, for example by rapidly skipping over matched
    // delimiters without paying close attention to the data in between.
    //
    // Some formats are not able to implement this at all. Formats that can
    // implement `deserialize_any` and `deserialize_ignored_any` are known as
    // self-describing.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}
// }}}

// NestedAccess {{{
struct NestedAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    idx: usize,
    hash_iter: Option<std::collections::hash_map::Iter<'de, String, JsValue>>,
}

impl<'a, 'de> NestedAccess<'a, 'de> {
    pub fn new(de: &'a mut Deserializer<'de>) -> Self {
        NestedAccess {
            de,
            idx: 0,
            hash_iter: None,
        }
    }
}

// impl SeqAccess for NestedAccess {{{
impl<'de, 'a> SeqAccess<'de> for NestedAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let vec = match self.de.pending_js_value.last() {
            Some(JsValue::Array(vec)) => vec,
            _ => todo!(),
        };
        if self.idx >= vec.len() {
            Ok(None)
        } else {
            self.de.pending_js_value.push(&vec[self.idx]);
            self.idx += 1;
            let item = seed.deserialize(&mut *self.de)?; //vec[self.idx];
            self.de.pending_js_value.pop();
            Ok(Some(item))
        }
    }
}
// }}}

// impl MapAccess for NestedAccess {{{
impl<'de, 'a> MapAccess<'de> for NestedAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        let map = match self.de.pending_js_value.last() {
            Some(JsValue::Object(map)) => map,
            _ => return Err(Error::Message("Expected JsValue::Object".into())),
        };
        let mut iter = match self.hash_iter.as_mut() {
            Some(i) => i.peekable(),
            None => {
                let i = map.iter();
                self.hash_iter = Some(i);
                self.hash_iter.as_mut().unwrap().peekable()
            }
        };
        match iter.peek() {
            Some((k, _)) => {
                let k = (*k).clone();
                let result = seed.deserialize(k.into_deserializer())?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let map = match self.de.pending_js_value.last() {
            Some(JsValue::Object(map)) => map,
            _ => unreachable!(),
        };
        let mut iter = match self.hash_iter.as_mut() {
            Some(i) => i,
            None => unreachable!(),
        };
        let value = match iter.next() {
            Some((_k, v)) => v,
            None => unreachable!(),
        };
        self.de.pending_js_value.push(value);
        let result = seed.deserialize(&mut *self.de)?;
        self.de.pending_js_value.pop();
        Ok(result)
    }
}
// }}}
// }}}

// de tests {{{
#[test]
fn test_de_primitives() {
    let i: i8 = from_js_value(&JsValue::Int(12)).unwrap();
    assert_eq!(i, 12);

    let f: f32 = from_js_value(&JsValue::Int(12)).unwrap();
    assert_eq!(f, 12f32);

    let f: f32 = from_js_value(&JsValue::Float(3.14f64)).unwrap();
    assert_eq!(f, 3.14f32);

    let b: bool = from_js_value(&JsValue::Bool(true)).unwrap();
    assert_eq!(b, true);

    let j = JsValue::String("a".into());
    let s: String = from_js_value(&j).unwrap();
    assert_eq!(s, "a".to_string());
}

#[test]
fn test_de_struct() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Test {
        int: u32,
        seq: Vec<String>,
    }

    let mut map = HashMap::new();
    map.insert("int".into(), JsValue::Int(1));
    map.insert(
        "seq".into(),
        JsValue::Array(vec![
            JsValue::String("a".into()),
            JsValue::String("b".into()),
        ]),
    );
    let j = JsValue::Object(map);
    let expected = Test {
        int: 1,
        seq: vec!["a".to_owned(), "b".to_owned()],
    };
    assert_eq!(expected, from_js_value(&j).unwrap());
}
//
// #[test]
// fn test_enum() {
//     #[derive(Deserialize, PartialEq, Debug)]
//     enum E {
//         Unit,
//         Newtype(u32),
//         Tuple(u32, u32),
//         Struct { a: u32 },
//     }
//
//     let j = r#""Unit""#;
//     let expected = E::Unit;
//     assert_eq!(expected, from_js_value(j).unwrap());
//
//     let j = r#"{"Newtype":1}"#;
//     let expected = E::Newtype(1);
//     assert_eq!(expected, from_js_value(j).unwrap());
//
//     let j = r#"{"Tuple":[1,2]}"#;
//     let expected = E::Tuple(1, 2);
//     assert_eq!(expected, from_js_value(j).unwrap());
//
//     let j = r#"{"Struct":{"a":1}}"#;
//     let expected = E::Struct { a: 1 };
//     assert_eq!(expected, from_js_value(j).unwrap());
// }
// }}}
// }}}
