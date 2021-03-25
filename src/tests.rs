use std::collections::HashMap;

use super::*;

// #[test]
// fn test_global_properties() {
//     let c = Context::new().unwrap();

//     assert_eq!(
//         c.global_property("lala"),
//         Err(ExecutionError::Exception(
//             "Global object does not have property 'lala'".into()
//         ))
//     );

//     c.set_global_property("testprop", true).unwrap();
//     assert_eq!(
//         c.global_property("testprop").unwrap(),
//         JsValue::Bool(true),
//     );
// }

#[test]
fn test_eval_pass() {
    use std::iter::FromIterator;

    let c = Context::new().unwrap();

    let cases = vec![
        ("undefined", Ok(JsValue::Undefined)),
        ("null", Ok(JsValue::Null)),
        ("true", Ok(JsValue::Bool(true))),
        ("2 > 10", Ok(JsValue::Bool(false))),
        ("1", Ok(JsValue::Int(1))),
        ("1 + 1", Ok(JsValue::Int(2))),
        ("1.1", Ok(JsValue::Float(1.1))),
        ("2.2 * 2 + 5", Ok(JsValue::Float(9.4))),
        ("\"abc\"", Ok(JsValue::String("abc".into()))),
        (
            "[1,2]",
            Ok(JsValue::Array(vec![JsValue::Int(1), JsValue::Int(2)])),
        ),
    ];

    for (code, res) in cases.into_iter() {
        assert_eq!(c.eval(code), res,);
    }

    let obj_cases = vec![
        (
            r#" {"a": null, "b": undefined} "#,
            Ok(JsValue::Object(HashMap::from_iter(vec![
                ("a".to_string(), JsValue::Null),
                ("b".to_string(), JsValue::Undefined),
            ]))),
        ),
        (
            r#" {a: 1, b: true, c: {c1: false}} "#,
            Ok(JsValue::Object(HashMap::from_iter(vec![
                ("a".to_string(), JsValue::Int(1)),
                ("b".to_string(), JsValue::Bool(true)),
                (
                    "c".to_string(),
                    JsValue::Object(HashMap::from_iter(vec![(
                        "c1".to_string(),
                        JsValue::Bool(false),
                    )])),
                ),
            ]))),
        ),
    ];

    for (index, (code, res)) in obj_cases.into_iter().enumerate() {
        let full_code = format!(
            "var v{index} = {code}; v{index}",
            index = index,
            code = code
        );
        assert_eq!(c.eval(&full_code), res,);
    }

    assert_eq!(c.eval_as::<bool>("true").unwrap(), true,);
    assert_eq!(c.eval_as::<i32>("1 + 2").unwrap(), 3,);

    let value: String = c.eval_as("var x = 44; x.toString()").unwrap();
    assert_eq!(&value, "44");

    #[cfg(feature = "bigint")]
    assert_eq!(
        c.eval_as::<num_bigint::BigInt>("1n << 100n").unwrap(),
        num_bigint::BigInt::from(1i128 << 100)
    );

    #[cfg(feature = "bigint")]
    assert_eq!(c.eval_as::<i64>("1 << 30").unwrap(), 1i64 << 30);

    #[cfg(feature = "bigint")]
    assert_eq!(c.eval_as::<u128>("1n << 100n").unwrap(), 1u128 << 100);
}

#[test]
fn test_eval_syntax_error() {
    let c = Context::new().unwrap();
    assert_eq!(
        c.eval(
            r#"
            !!!!
        "#
        ),
        Err(ExecutionError::Exception(
            "SyntaxError: unexpected token in expression: \'\'".into()
        ))
    );
}

#[test]
fn test_eval_exception() {
    let c = Context::new().unwrap();
    assert_eq!(
        c.eval(
            r#"
            function f() {
                throw new Error("My Error");
            }
            f();
        "#
        ),
        Err(ExecutionError::Exception("Error: My Error".into(),))
    );
}

#[test]
fn eval_async() {
    let c = Context::new().unwrap();

    let value = c
        .eval(
            r#"
        new Promise((resolve, _) => {
            resolve(33);
        })
    "#,
        )
        .unwrap();
    assert_eq!(value, JsValue::Int(33));

    let res = c.eval(
        r#"
        new Promise((_resolve, reject) => {
            reject("Failed...");
        })
    "#,
    );
    assert_eq!(
        res,
        Err(ExecutionError::Exception(JsValue::String(
            "Failed...".into()
        )))
    );
}

#[test]
fn test_set_global() {
    let context = Context::new().unwrap();
    context.set_global("someGlobalVariable", 42).unwrap();
    let value = context.eval_as::<i32>("someGlobalVariable").unwrap();
    assert_eq!(value, 42,);
}

#[test]
fn test_call() {
    let c = Context::new().unwrap();

    assert_eq!(
        c.call_function("parseInt", vec!["22"]).unwrap(),
        JsValue::Int(22),
    );

    c.eval(
        r#"
        function add(a, b) {
            return a + b;
        }
    "#,
    )
    .unwrap();
    assert_eq!(
        c.call_function("add", vec![5, 7]).unwrap(),
        JsValue::Int(12),
    );

    c.eval(
        r#"
        function sumArray(arr) {
            let sum = 0;
            for (const value of arr) {
                sum += value;
            }
            return sum;
        }
    "#,
    )
    .unwrap();
    assert_eq!(
        c.call_function("sumArray", vec![vec![1, 2, 3]]).unwrap(),
        JsValue::Int(6),
    );

    c.eval(
        r#"
        function addObject(obj) {
            let sum = 0;
            for (const key of Object.keys(obj)) {
                sum += obj[key];
            }
            return sum;
        }
    "#,
    )
    .unwrap();
    let mut obj = std::collections::HashMap::<String, i32>::new();
    obj.insert("a".into(), 10);
    obj.insert("b".into(), 20);
    obj.insert("c".into(), 30);
    assert_eq!(
        c.call_function("addObject", vec![obj]).unwrap(),
        JsValue::Int(60),
    );
}

#[test]
fn test_call_large_string() {
    let c = Context::new().unwrap();
    c.eval(" function strLen(s) { return s.length; } ").unwrap();

    let s = " ".repeat(200_000);
    let v = c.call_function("strLen", vec![s]).unwrap();
    assert_eq!(v, JsValue::Int(200_000));
}

#[test]
fn call_async() {
    let c = Context::new().unwrap();

    c.eval(
        r#"
        function asyncOk() {
            return new Promise((resolve, _) => {
                resolve(33);
            });
        }

        function asyncErr() {
            return new Promise((_resolve, reject) => {
                reject("Failed...");
            });
        }
    "#,
    )
    .unwrap();

    let value = c.call_function("asyncOk", vec![true]).unwrap();
    assert_eq!(value, JsValue::Int(33));

    let res = c.call_function("asyncErr", vec![true]);
    assert_eq!(
        res,
        Err(ExecutionError::Exception(JsValue::String(
            "Failed...".into()
        )))
    );
}

#[test]
fn test_callback() {
    let c = Context::new().unwrap();

    c.add_callback("no_arguments", || true).unwrap();
    assert_eq!(c.eval_as::<bool>("no_arguments()").unwrap(), true);

    c.add_callback("cb1", |flag: bool| !flag).unwrap();
    assert_eq!(c.eval("cb1(true)").unwrap(), JsValue::Bool(false),);

    c.add_callback("concat2", |a: String, b: String| format!("{}{}", a, b))
        .unwrap();
    assert_eq!(
        c.eval(r#"concat2("abc", "def")"#).unwrap(),
        JsValue::String("abcdef".into()),
    );

    c.add_callback("add2", |a: i32, b: i32| -> i32 { a + b })
        .unwrap();
    assert_eq!(c.eval("add2(5, 11)").unwrap(), JsValue::Int(16),);

    c.add_callback("sum", |items: Vec<i32>| -> i32 { items.iter().sum() })
        .unwrap();
    assert_eq!(c.eval("sum([1, 2, 3, 4, 5, 6])").unwrap(), JsValue::Int(21),);

    c.add_callback("identity", |value: JsValue| -> JsValue { value })
        .unwrap();
    {
        let v = JsValue::from(22);
        assert_eq!(c.eval("identity(22)").unwrap(), v);
    }
}

#[test]
fn test_callback_argn_variants() {
    macro_rules! callback_argn_tests {
        [
            $(
                $len:literal : ( $( $argn:ident : $argv:literal ),* ),
            )*
        ] => {
            $(
                {
                    // Test plain return type.
                    let name = format!("cb{}", $len);
                    let c = Context::new().unwrap();
                    c.add_callback(&name, | $( $argn : i32 ),*| -> i32 {
                        $( $argn + )* 0
                    }).unwrap();

                    let code = format!("{}( {} )", name, "1,".repeat($len));
                    let v = c.eval(&code).unwrap();
                    assert_eq!(v, JsValue::Int($len));

                    // Test Result<T, E> return type with OK(_) returns.
                    let name = format!("cbres{}", $len);
                    c.add_callback(&name, | $( $argn : i32 ),*| -> Result<i32, String> {
                        Ok($( $argn + )* 0)
                    }).unwrap();

                    let code = format!("{}( {} )", name, "1,".repeat($len));
                    let v = c.eval(&code).unwrap();
                    assert_eq!(v, JsValue::Int($len));

                    // Test Result<T, E> return type with Err(_) returns.
                    let name = format!("cbreserr{}", $len);
                    c.add_callback(&name, #[allow(unused_variables)] | $( $argn : i32 ),*| -> Result<i32, String> {
                        Err("error".into())
                    }).unwrap();

                    let code = format!("{}( {} )", name, "1,".repeat($len));
                    let res = c.eval(&code);
                    assert_eq!(res, Err(ExecutionError::Exception("error".into())));
                }
            )*
        }
    }

    callback_argn_tests![
        1: (a : 1),
    ]
}

#[test]
fn test_callback_varargs() {
    let c = Context::new().unwrap();

    // No return.
    c.add_callback("cb", |args: Arguments| {
        let args = args.into_vec();
        assert_eq!(
            args,
            vec![
                JsValue::String("hello".into()),
                JsValue::Bool(true),
                JsValue::from(100),
            ]
        );
    })
    .unwrap();
    assert_eq!(
        c.eval_as::<bool>("cb('hello', true, 100) === undefined")
            .unwrap(),
        true
    );

    // With return.
    c.add_callback("cb2", |args: Arguments| -> u32 {
        let args = args.into_vec();
        assert_eq!(
            args,
            vec![JsValue::from(1), JsValue::from(10), JsValue::from(100),]
        );
        111
    })
    .unwrap();
    c.eval(
        r#"
        var x = cb2(1, 10, 100);
        if (x !== 111) {
        throw new Error('Expected 111, got ' + x);
        }
    "#,
    )
    .unwrap();
}

#[test]
fn test_callback_invalid_argcount() {
    let c = Context::new().unwrap();

    c.add_callback("cb", |a: i32, b: i32| a + b).unwrap();

    assert_eq!(
        c.eval(" cb(5) "),
        Err(ExecutionError::Exception(
            "Invalid argument count: Expected 2, got 1".into()
        )),
    );
}

#[test]
fn memory_limit_exceeded() {
    let c = Context::builder().memory_limit(100_000).build().unwrap();
    assert_eq!(
        c.eval("  'abc'.repeat(200_000) "),
        Err(ExecutionError::OutOfMemory),
    );
}

#[test]
fn context_reset() {
    let c = Context::new().unwrap();
    c.eval(" var x = 123; ").unwrap();
    c.add_callback("myCallback", || true).unwrap();

    let c2 = c.reset().unwrap();

    // Check it still works.
    assert_eq!(
        c2.eval_as::<String>(" 'abc'.repeat(2) ").unwrap(),
        "abcabc".to_string(),
    );

    // Check old state is gone.
    let err_msg = c2.eval(" x ").unwrap_err().to_string();
    assert!(err_msg.contains("ReferenceError"));

    // Check callback is gone.
    let err_msg = c2.eval(" myCallback() ").unwrap_err().to_string();
    assert!(err_msg.contains("ReferenceError"));
}

#[inline(never)]
fn build_context() -> Context {
    let ctx = Context::new().unwrap();
    let name = "cb".to_string();
    ctx.add_callback(&name, |a: String| a.repeat(2)).unwrap();

    let code = " function f(value) { return cb(value); } ".to_string();
    ctx.eval(&code).unwrap();

    ctx
}

#[test]
fn moved_context() {
    let c = build_context();
    let v = c.call_function("f", vec!["test"]).unwrap();
    assert_eq!(v, "testtest".into());

    let v = c.eval(" f('la') ").unwrap();
    assert_eq!(v, "lala".into());
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_serialize() {
    let c = build_context();

    c.eval(
        "
        function dateToTimestamp(date) {
            return date.getTime();
        }
    ",
    )
    .unwrap();

    let now = chrono::Utc::now();
    let now_millis = now.timestamp_millis();

    let timestamp = c
        .call_function("dateToTimestamp", vec![JsValue::Date(now.clone())])
        .unwrap();

    assert_eq!(timestamp, JsValue::Float(now_millis as f64));
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_deserialize() {
    use chrono::offset::TimeZone;

    let c = build_context();

    let value = c.eval(" new Date(1234567555) ").unwrap();
    let datetime = chrono::Utc.timestamp_millis(1234567555);

    assert_eq!(value, JsValue::Date(datetime));
}

#[cfg(feature = "chrono")]
#[test]
fn chrono_roundtrip() {
    let c = build_context();

    c.eval(" function identity(x) { return x; } ").unwrap();
    let d = chrono::Utc::now();
    let td = JsValue::Date(d.clone());
    let td2 = c.call_function("identity", vec![td.clone()]).unwrap();
    let d2 = if let JsValue::Date(x) = td2 {
        x
    } else {
        panic!("expected date")
    };

    assert_eq!(d.timestamp_millis(), d2.timestamp_millis());
}

#[cfg(feature = "bigint")]
#[test]
fn test_bigint_deserialize_i64() {
    for i in vec![0, std::i64::MAX, std::i64::MIN] {
        let c = Context::new().unwrap();
        let value = c.eval(&format!("{}n", i)).unwrap();
        assert_eq!(value, JsValue::BigInt(i.into()));
    }
}

#[cfg(feature = "bigint")]
#[test]
fn test_bigint_deserialize_bigint() {
    for i in vec![
        std::i64::MAX as i128 + 1,
        std::i64::MIN as i128 - 1,
        std::i128::MAX,
        std::i128::MIN,
    ] {
        let c = Context::new().unwrap();
        let value = c.eval(&format!("{}n", i)).unwrap();
        let expected = num_bigint::BigInt::from(i);
        assert_eq!(value, JsValue::BigInt(expected.into()));
    }
}

#[cfg(feature = "bigint")]
#[test]
fn test_bigint_serialize_i64() {
    for i in vec![0, std::i64::MAX, std::i64::MIN] {
        let c = Context::new().unwrap();
        c.eval(&format!(" function isEqual(x) {{ return x === {}n }} ", i))
            .unwrap();
        assert_eq!(
            c.call_function("isEqual", vec![JsValue::BigInt(i.into())])
                .unwrap(),
            JsValue::Bool(true)
        );
    }
}

#[cfg(feature = "bigint")]
#[test]
fn test_bigint_serialize_bigint() {
    for i in vec![
        std::i64::MAX as i128 + 1,
        std::i64::MIN as i128 - 1,
        std::i128::MAX,
        std::i128::MIN,
    ] {
        let c = Context::new().unwrap();
        c.eval(&format!(" function isEqual(x) {{ return x === {}n }} ", i))
            .unwrap();
        let value = JsValue::BigInt(num_bigint::BigInt::from(i).into());
        assert_eq!(
            c.call_function("isEqual", vec![value]).unwrap(),
            JsValue::Bool(true)
        );
    }
}

#[test]
fn test_console() {
    use console::Level;
    use std::sync::{Arc, Mutex};

    let messages = Arc::new(Mutex::new(Vec::<(Level, Vec<JsValue>)>::new()));

    let m = messages.clone();
    let c = Context::builder()
        .console(move |level: Level, args: Vec<JsValue>| {
            m.lock().unwrap().push((level, args));
        })
        .build()
        .unwrap();

    c.eval(
        r#"
        console.log("hi");
        console.error(false);
    "#,
    )
    .unwrap();

    let m = messages.lock().unwrap();

    assert_eq!(
        *m,
        vec![
            (Level::Log, vec![JsValue::from("hi")]),
            (Level::Error, vec![JsValue::from(false)]),
        ]
    );
}

#[test]
fn test_global_setter() {
    let ctx = Context::new().unwrap();
    ctx.set_global("a", "a").unwrap();
    ctx.eval("a + 1").unwrap();
}
