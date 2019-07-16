use std::path::{Path, PathBuf};

use std::env;

fn exists(path: impl AsRef<Path>) -> bool {
    PathBuf::from(path.as_ref()).exists()
}

#[cfg(all(not(feature = "system"), not(feature = "bundled")))]
fn main() {
    panic!("Invalid config for crate quickjs-sys: must enable either the 'bundled' or the 'system' feature");
}

#[cfg(feature = "system")]
extern crate bindgen;

#[cfg(feature = "system")]
fn main() {
    #[cfg(not(feature = "bindgen"))]
    panic!("Invalid configuration for quickjs-sys: Must either enable the bundled or the bindgen feature");


    let lib = if cfg!(unix) {
        if exists("/usr/lib/quickjs/libquickjs.a") {
            "/usr/lib/quickjs"
        } else if exists("/usr/local/lib/quickjs") {
            "/usr/local/lib/quickjs"
        } else {
            panic!("quicks is not supported on this platform");
        }
    } else {
            panic!("quickjs error: Windows is not supported yet");
    };

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

    // Instruct cargo to statically link quickjs.
    println!("cargo:rustc-link-search=native={}", lib);
    println!("cargo:rustc-link-lib=static=quickjs");
}

#[cfg(feature = "bundled")]
fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let code_dir = out_path.join("quickjs");
    if exists(&code_dir) {
        std::fs::remove_dir_all(&code_dir).unwrap();
    }
    copy_dir::copy_dir("./embed/quickjs", &code_dir).expect("Could not copy quickjs directory");

    eprintln!("Compiling quickjs...");
    std::process::Command::new("make")
        .arg("libquickjs.a")
        .current_dir(&code_dir)
        .spawn()
        .expect("Could not compile quickjs")
        .wait()
        .expect("Could not compile quickjs");

    std::fs::copy("./embed/bindings.rs", out_path.join("bindings.rs"))
        .expect("Could not copy bindings.rs");

    // Instruct cargo to statically link quickjs.
    println!("cargo:rustc-link-search=native={}", code_dir.to_str().unwrap());
    println!("cargo:rustc-link-lib=static=quickjs");
}
