# quickjs-rs

[![Crates.io](https://img.shields.io/crates/v/quick-js.svg?maxAge=3600)](https://crates.io/crates/quick-js)
[![docs.rs](https://docs.rs/quick-js/badge.svg)](https://docs.rs/quick-js)
[![Build Status](https://dev.azure.com/the-duke/quickjs-rs/_apis/build/status/theduke.quickjs-rs?branchName=master)](https://dev.azure.com/the-duke/quickjs-rs/_build/latest?definitionId=2&branchName=master)
[![CircleCI](https://circleci.com/gh/theduke/quickjs-rs.svg?style=svg)](https://circleci.com/gh/theduke/quickjs-rs)

A Rust wrapper for [QuickJS](https://bellard.org/quickjs/). 

QuickJS is a new, small Javascript engine by Fabrice Bellard. 
It is fast and supports the full ES2019 specification.

This crate allows you to easily run and integrate with Javascript code from Rust.

## Limitations

* JS objects can not be deserialized into Rust (JsValue::Object) due to a missing property enumeration API
    (will be fixed soon)
* Windows is not supported yet

## Usage

```toml
[dependencies]
quick-js = "0.2.0"
```

```rust
use quick_js::{Context, JsValue};

let mut context = Context::new().unwrap();

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

