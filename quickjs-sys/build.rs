use std::{path::PathBuf};

extern crate bindgen;

use std::env;

fn exists(path: &str) -> bool {
    PathBuf::from(path).exists()
}

fn find_lib() -> Option<String> {
    if cfg!(unix) {
        if exists("/usr/lib/quickjs/libquickjs.a") {
            Some("/usr/lib/quickjs".into())
        } else if exists("/usr/local/lib/quickjs") {
            Some("/usr/local/lib/quickjs".into())
        } else {
            None
        }
    } else {
        panic!("quicks is not supported on this platform");
    }
}

fn main() {
    let lib = find_lib().expect("Could not locate quickjs library. Is quickjs installed?");

    // Instruct cargo to statically link quickjs.
    println!("cargo:rustc-link-search=native={}", lib);
    println!("cargo:rustc-link-lib=static=quickjs");

    // Generate bindings.
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
