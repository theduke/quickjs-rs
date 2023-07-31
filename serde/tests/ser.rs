use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

use quick_js::{Context, ExecutionError};
use serde::ser::{Error, SerializeMap};
use serde::{Serialize, Serializer};

fn run_serialize_error<T>(value: &T) -> ExecutionError
where
    T: Serialize,
{
    let context = Context::new().unwrap();

    let result = context.set_global_serde("example", value);

    result.expect_err("serialization should fail")
}

fn run<T>(value: &T) -> String
where
    T: Serialize,
{
    let context = Context::new().unwrap();

    context.set_global_serde("example", value).unwrap();

    context
        .eval_as::<String>("JSON.stringify(example)")
        .unwrap()
}

#[test]
fn u8() {
    assert_eq!(run(&5u8), "5");
}

#[test]
fn u16() {
    assert_eq!(run(&5u16), "5");
}

#[test]
fn u32() {
    assert_eq!(run(&5u32), "5");
}

#[test]
fn u64() {
    assert_eq!(run(&5u64), "5");
}

#[test]
fn u128() {
    assert_eq!(run(&5u128), "5");
}

#[test]
fn i8() {
    assert_eq!(run(&-5i8), "-5");
}

#[test]
fn i16() {
    assert_eq!(run(&-5i16), "-5");
}

#[test]
fn i32() {
    assert_eq!(run(&-5i32), "-5");
}

#[test]
fn i64() {
    assert_eq!(run(&-5i64), "-5");
}

#[test]
fn i128() {
    assert_eq!(run(&-5i128), "-5");
}

#[test]
fn bool() {
    assert_eq!(run(&true), "true");
    assert_eq!(run(&false), "false");
}

#[test]
fn char() {
    assert_eq!(run(&'a'), r#""a""#);
}

#[test]
fn str() {
    assert_eq!(run(&"abc"), r#""abc""#);
}

#[test]
fn string() {
    assert_eq!(run(&String::from("abc")), r#""abc""#);
}

#[test]
fn unit() {
    assert_eq!(run(&()), "null");
}

#[test]
fn option() {
    assert_eq!(run(&Some(5u8)), "5");
    assert_eq!(run(&None::<u8>), "null");
}

#[test]
fn vec() {
    assert_eq!(run(&vec![5u8, 6u8]), "[5,6]");
}

#[test]
fn tuple() {
    assert_eq!(run(&(5u8, 6u8)), "[5,6]");
}

#[test]
fn tuple_struct() {
    #[derive(Serialize)]
    struct TupleStruct(u8, u8);

    assert_eq!(run(&TupleStruct(5u8, 6u8)), "[5,6]");
}

#[test]
fn map() {
    use std::collections::BTreeMap;

    let mut map = BTreeMap::new();
    map.insert("a", 5u8);
    map.insert("b", 6u8);

    assert_eq!(run(&map), r#"{"a":5,"b":6}"#);
}

#[test]
fn struct_() {
    #[derive(Serialize)]
    struct Struct {
        a: u8,
        b: u8,
    }

    assert_eq!(run(&Struct { a: 5u8, b: 6u8 }), r#"{"a":5,"b":6}"#);
}

#[test]
fn struct_with_lifetime() {
    #[derive(Serialize)]
    struct Struct<'a> {
        a: &'a str,
        b: &'a str,
    }

    assert_eq!(
        run(&Struct { a: "abc", b: "def" }),
        r#"{"a":"abc","b":"def"}"#
    );
}

#[test]
fn struct_with_lifetime_and_lifetime_in_type() {
    #[derive(Serialize)]
    struct Struct<'a> {
        a: &'a str,
        b: &'a str,
        c: std::marker::PhantomData<&'a ()>,
    }

    assert_eq!(
        run(&Struct {
            a: "abc",
            b: "def",
            c: std::marker::PhantomData,
        }),
        r#"{"a":"abc","b":"def","c":null}"#
    );
}

#[test]
fn zero_sized_struct() {
    #[derive(Serialize)]
    struct Struct;

    assert_eq!(run(&Struct), r#"null"#);
}

#[test]
fn enum_() {
    #[derive(Serialize)]
    enum Enum {
        A,
        B,
    }

    assert_eq!(run(&Enum::A), r#""A""#);
    assert_eq!(run(&Enum::B), r#""B""#);
}

#[test]
fn enum_tuple() {
    #[derive(Serialize)]
    enum Enum {
        A(u8),
        B(u8),
    }

    assert_eq!(run(&Enum::A(5u8)), r#"{"A":5}"#);
    assert_eq!(run(&Enum::B(6u8)), r#"{"B":6}"#);
}

#[test]
fn enum_struct() {
    #[derive(Serialize)]
    enum Enum {
        A { a: u8 },
        B { b: u8 },
    }

    assert_eq!(run(&Enum::A { a: 5u8 }), r#"{"A":{"a":5}}"#);
    assert_eq!(run(&Enum::B { b: 6u8 }), r#"{"B":{"b":6}}"#);
}

#[test]
fn enum_with_lifetime() {
    #[derive(Serialize)]
    enum Enum<'a> {
        A { a: &'a str },
        B { b: &'a str },
    }

    assert_eq!(run(&Enum::A { a: "abc" }), r#"{"A":{"a":"abc"}}"#);
    assert_eq!(run(&Enum::B { b: "def" }), r#"{"B":{"b":"def"}}"#);
}

#[test]
fn enum_with_lifetime_and_lifetime_in_type() {
    #[derive(Serialize)]
    enum Enum<'a> {
        A { a: &'a str },
        B { b: &'a str },
        C(PhantomData<&'a ()>),
    }

    assert_eq!(run(&Enum::A { a: "abc" }), r#"{"A":{"a":"abc"}}"#);
    assert_eq!(run(&Enum::B { b: "def" }), r#"{"B":{"b":"def"}}"#);
}

#[test]
fn vec_element_error() {
    struct Element;

    static COUNT: AtomicUsize = AtomicUsize::new(0);

    impl Serialize for Element {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let next = COUNT.fetch_add(1, Ordering::SeqCst);

            if next == 1 {
                Err(Error::custom("failure"))
            } else {
                next.serialize(serializer)
            }
        }
    }

    assert_eq!(
        run_serialize_error(&vec![Element, Element, Element]),
        ExecutionError::Serialize
    );
}

#[test]
fn map_key_error() {
    struct Key;

    impl Serialize for Key {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&Key)?;
            map.serialize_value(&5u8)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_value_error() {
    struct Value;

    impl Serialize for Value {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&5u8)?;
            map.serialize_value(&Value)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_entry_key_error() {
    struct Key;

    impl Serialize for Key {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(&Key, &5u8)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_entry_value_error() {
    struct Value;

    impl Serialize for Value {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(&5u8, &Value)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_entry_error() {
    struct Key;

    impl Serialize for Key {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Value;

    impl Serialize for Value {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(Error::custom("failure"))
        }
    }

    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(&Key, &Value)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_no_corresponding_value_error() {
    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&5u8)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}

#[test]
fn map_extra_value_error() {
    struct Map;

    impl Serialize for Map {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_key(&5u8)?;
            map.serialize_value(&5u8)?;
            map.serialize_value(&5u8)?;
            map.end()
        }
    }

    assert_eq!(run_serialize_error(&Map), ExecutionError::Serialize);
}
