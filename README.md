# quickjs-rs

[![Crates.io](https://img.shields.io/crates/v/quick-js.svg?maxAge=3600)](https://crates.io/crates/quick-js)
[![docs.rs](https://docs.rs/quick-js/badge.svg)](https://docs.rs/quick-js)
[![Build Status](https://github.com/theduke/quickjs-rs/workflows/CI/badge.svg)

A Rust wrapper for [QuickJS](https://bellard.org/quickjs/). 

QuickJS is a new, small Javascript engine by Fabrice Bellard and Charlie Gordon. 
It is fast and supports the full ES2020 specification.

This crate allows you to easily run and integrate with Javascript code from Rust.

## Quickstart

```toml
[dependencies]
quick-js = "0.4.1"
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

## Optional Features

The crate supports the following features:

* `chrono`: chrono integration
    - adds a `JsValue::Date` variant that can be (de)serialized to/from a JS `Date`
* `bigint`: arbitrary precision integer support via [num-bigint](https://github.com/rust-num/num-bigint)
* `log`: allows forwarding `console.log` messages to the `log` crate.
    Note: must be enabled with `ContextBuilder::console(quick_js::console::LogConsole);`

* `patched` 
    Enabled automatically for some other features, like `bigint`. 
    You should not need to enable this manually.
    Applies QuickJS patches that can be found in `libquickjs-sys/embed/patches` directory.


## Installation

By default, quickjs is **bundled** with the `libquickjs-sys` crate and
automatically compiled, assuming you have the appropriate dependencies.

### Windows Support

Windows is only supported with the [MSYS2](https://www.msys2.org/) environment 
and `x86_64-pc-windows-gnu` target architecture. 

If you have MSYS2 installed and the MSYS `bin` directory in your path, you can
compile quickjs with `cargo build --target="x86_64-pc-windows-gnu"`. 

The target can also be configured permanently via a 
[cargo config file](https://doc.rust-lang.org/cargo/reference/config.html) or 
the `CARGO_BUILD_TARGET` env var.

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

You then need to disable the `bundled` feature in the `libquickjs-sys` crate to
force using the system version.
