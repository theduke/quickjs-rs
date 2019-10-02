# quickjs-rs

[![Crates.io](https://img.shields.io/crates/v/quick-js.svg?maxAge=3600)](https://crates.io/crates/quick-js)
[![docs.rs](https://docs.rs/quick-js/badge.svg)](https://docs.rs/quick-js)
[![Build Status](https://github.com/theduke/quickjs-rs/workflows/CI/badge.svg)

A Rust wrapper for [QuickJS](https://bellard.org/quickjs/). 

QuickJS is a new, small Javascript engine by Fabrice Bellard and Charlie Gordon. 
It is fast and supports the full ES2020 specification.

This crate allows you to easily run and integrate with Javascript code from Rust.

## Limitations

* Windows is not supported yet

## Quickstart

```toml
[dependencies]
quick-js = "0.2.2"
```

```rust
use quick_js::{Context, JsValue};

let context = Context::new().unwrap();

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

## Installation

By default, quickjs is **bundled** with the `libquickjs-sys` crate and
automatically compiled, assuming you have the appropriate dependencies.

If you would like to use a system version instead, see below. 

QuickJS will always be statically linked to your binary.

### Features

The crate supports the following features:

* `chrono`: adds chrono integration
    - adds a `JsValue::Date` variant that can be (de)serialized to/from a JS `Date`
* `bigint`: arbitrary precision integer support via [num-bigint](https://github.com/rust-num/num-bigint)
* `patched`: applies QuickJS patches that can be found in `libquickjs-sys/embed/patches` directory.
* `log`: allows forwarding `console.log` messages to the `log` crate.
    Note: must be enabled with `ContextBuilder::console(quickjs::console::LogConsole);`

### System installation

To use the system installation, without the bundled feature, first install the required 
dependencies, and then compile and install quickjs.

```bash
# Debian/Ubuntu: apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang
mkdir quickjs 
curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C quickjs --strip-components 1
cd quickjs
sudo make install
```

