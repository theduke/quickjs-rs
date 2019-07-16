# quickjs-rs

A Rust wrapper for [quickjs](https://bellard.org/quickjs/), a Javascript engine.

This crate allows you to easily run ES2019 based Javascript code from a Rust context.

## Installation


To use this crate, `quickjs` must be installed on the system.

```bash
mkdir quickjs 
curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C quickjs --strip-components 1
cd quickjs
sudo make install
```

Then just add `quickjs` as a dependency.

## Usage

```rust
use quickjs::{Context, JsValue};

let context = quickjs::Context::new().unwrap();

// Eval.

let value = context.eval("1 + 2").unwrap();
assert_eq!(value, JsValue::Int(3));

let value: String = context.eval(" var x = 100 + 250; x.toString() ").unwrap();
assert_eq!(&value, "350");

// Callbacks.

context.add_callback("myCallback", |a: i32, b: i32| a + b).unwrap();

context.eval(r#"
    // x will equal 30
    var x = myCallback(10, 20);
"#).unwrap();

```
