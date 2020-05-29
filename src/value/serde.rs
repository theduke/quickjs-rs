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
        let value = value.serialize(&mut **self)?;
        match &mut self.pending_js_value.last_mut() {
            Some(JsValue::Array(arr)) => {
                arr.push(value);
                Ok(())
            }
            _ => return Err(Error::Message("Inner pending value is not an array".into())),
        }
    }

    fn end(self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending value is None".into()))
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
        let value = value.serialize(&mut **self)?;
        match &mut self.pending_js_value.last_mut() {
            Some(JsValue::Array(arr)) => {
                arr.push(value);
                Ok(())
            }
            _ => return Err(Error::Message("Inner pending value is not an array".into())),
        }
    }

    fn end(self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending value is None".into()))
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
        // if !self.output.ends_with('[') {
        //     self.output += ",";
        // }
        let value = value.serialize(&mut **self)?;
        match self.pending_js_value.last_mut() {
            Some(JsValue::Array(arr)) => {
                arr.push(value);
                Ok(())
            }
            _ => return Err(Error::Message("Inner pending value is not an array".into())),
        }
    }

    fn end(self) -> Result<JsValue> {
        let value = self
            .pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending value is None".into()))?;
        let mut map = HashMap::new();
        map.insert(
            self.pending_hashmap_key
                .take()
                .ok_or(Error::Message("Inner pending HashMap key is None".into()))?,
            value,
        );
        Ok(JsValue::Object(map))
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
        let key = key.serialize(&mut **self)?;
        let key_as_string = match key {
            JsValue::String(s) => s,
            _ => return Err(Error::Message("Expected key to be a string".into())),
        };
        let value = value.serialize(&mut **self)?;
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

    fn end(self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending object is None".into()))
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
        let value = value.serialize(&mut **self)?;
        match &mut self.pending_js_value.last_mut() {
            Some(JsValue::Object(map)) => {
                map.insert(key.into(), value);
                Ok(())
            }
            _ => {
                return Err(Error::Message(
                    "Inner pending object is not a JsValue::Object".into(),
                ))
            }
        }
    }

    fn end(self) -> Result<JsValue> {
        self.pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending object is None".into()))
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
        let value = value.serialize(&mut **self)?;
        match self.pending_js_value.last_mut() {
            Some(JsValue::Object(map)) => {
                map.insert(key.into(), value);
                Ok(())
            }
            _ => {
                return Err(Error::Message(
                    "Inner pending value is not an object".into(),
                ))
            }
        }
    }

    fn end(self) -> Result<JsValue> {
        let key = self
            .pending_hashmap_key
            .take()
            .ok_or(Error::Message("The pending HashMap key is None".into()))?;
        let value = self
            .pending_js_value
            .pop()
            .ok_or(Error::Message("Inner pending value is None".into()))?;
        let mut map = HashMap::new();
        map.insert(key, value);
        Ok(JsValue::Object(map))
    }
}
// }}}

// ser tests {{{
#[test]
fn test_struct() {
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
fn test_enum() {
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
    input: &'de JsValue,
}

impl<'de> Deserializer<'de> {
    pub fn from_js_value(input: &'de JsValue) -> Self {
        Deserializer { input }
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
        // match self.peek_char()? {
        //     'n' => self.deserialize_unit(visitor),
        //     't' | 'f' => self.deserialize_bool(visitor),
        //     '"' => self.deserialize_str(visitor),
        //     '0'..='9' => self.deserialize_u64(visitor),
        //     '-' => self.deserialize_i64(visitor),
        //     '[' => self.deserialize_seq(visitor),
        //     '{' => self.deserialize_map(visitor),
        //     _ => Err(Error::Syntax),
        // }
        todo!()
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(todo!())
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(todo!())
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(todo!())
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(todo!())
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(todo!())
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(todo!())
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(todo!())
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(todo!())
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(todo!())
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(todo!())
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        unimplemented!()
    }

    // An absent optional is represented as the JSON `null` and a present
    // optional is represented as just the contained value.
    //
    // As commented in `Serializer` implementation, this is a lossy
    // representation. For example the values `Some(())` and `None` both
    // serialize as just `null`. Unfortunately this is typically what people
    // expect when working with JSON. Other formats are encouraged to behave
    // more intelligently if possible.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // if self.input.starts_with("null") {
        //     self.input = &self.input["null".len()..];
        //     visitor.visit_none()
        // } else {
        //     visitor.visit_some(self)
        // }
        todo!()
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // if self.input.starts_with("null") {
        //     self.input = &self.input["null".len()..];
        //     visitor.visit_unit()
        // } else {
        //     Err(Error::ExpectedNull)
        // }
        todo!()
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
        // // Parse the opening bracket of the sequence.
        // if self.next_char()? == '[' {
        //     // Give the visitor access to each element of the sequence.
        //     let value = visitor.visit_seq(CommaSeparated::new(&mut self))?;
        //     // Parse the closing bracket of the sequence.
        //     if self.next_char()? == ']' {
        //         Ok(value)
        //     } else {
        //         Err(Error::ExpectedArrayEnd)
        //     }
        // } else {
        //     Err(Error::ExpectedArray)
        // }
        todo!()
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
        // // Parse the opening brace of the map.
        // if self.next_char()? == '{' {
        //     // Give the visitor access to each entry of the map.
        //     let value = visitor.visit_map(CommaSeparated::new(&mut self))?;
        //     // Parse the closing brace of the map.
        //     if self.next_char()? == '}' {
        //         Ok(value)
        //     } else {
        //         Err(Error::ExpectedMapEnd)
        //     }
        // } else {
        //     Err(Error::ExpectedMap)
        // }
        todo!()
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

// de tests {{{
// #[test]
// fn test_struct() {
//     #[derive(Deserialize, PartialEq, Debug)]
//     struct Test {
//         int: u32,
//         seq: Vec<String>,
//     }
//
//     let j = r#"{"int":1,"seq":["a","b"]}"#;
//     let expected = Test {
//         int: 1,
//         seq: vec!["a".to_owned(), "b".to_owned()],
//     };
//     assert_eq!(expected, from_js_value(j).unwrap());
// }
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
