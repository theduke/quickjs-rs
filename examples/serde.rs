use quick_js::Context;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Inner {
    b: u8,
}

#[derive(Debug, Serialize)]
pub struct Example {
    a: Vec<Inner>,
}

fn main() {
    let context = Context::new().unwrap();

    let value = context.eval("1 + 2").unwrap();
    println!("js: 1 + 2 = {:?}", value);

    context
        .add_callback("myCallback", |a: i32, b: i32| a + b * b)
        .unwrap();

    context
        .set_global_serde(
            "example",
            &Example {
                a: vec![Inner { b: 5 }, Inner { b: 6 }],
            },
        )
        .unwrap();

    let value = context
        .eval(
            r#"
       JSON.stringify(example)
"#,
        )
        .unwrap();
    println!("js: JSON.stringify(example) = {:?}", value);
}
