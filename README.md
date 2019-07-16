# quickjs-rs

[![Crates.io](https://img.shields.io/crates/v/quickjs.svg?maxAge=3600)](https://crates.io/crates/quickjs)
[![docs.rs](https://docs.rs/quickjs/badge.svg)](https://docs.rs/crates/quickjs)
[![Build Status](https://dev.azure.com/the-duke/quickjs-rs/_apis/build/status/theduke.quickjs-rs?branchName=master)](https://dev.azure.com/the-duke/quickjs-rs/_build/latest?definitionId=2&branchName=master)


A Rust wrapper for [quickjs](https://bellard.org/quickjs/), a Javascript engine.

This crate allows you to easily run ES2019 based Javascript code from a Rust context.

## Limitations / Warnings

* JS objects can not be deserialized into Rust (JsValue::Object) due to a missing property enumeration API
    (will be fixed soon)

## Installation


To use this crate, `quickjs` must be installed on the system.

```bash
# Debian/Ubuntu: apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang
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

let value = context.eval_as::<String>(" var x = 100 + 250; x.toString() ").unwrap();
assert_eq!(&value, "350");

// Callbacks.

context.add_callback("myCallback", |a: i32, b: i32| a + b).unwrap();

context.eval(r#"
    // x will equal 30
    var x = myCallback(10, 20);
"#).unwrap();
```
