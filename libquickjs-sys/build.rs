use std::path::PathBuf;

use std::env;

const LIB_NAME: &str = "quickjs";

#[cfg(all(not(feature = "system"), not(feature = "bundled")))]
fn main() {
    panic!("Invalid config for crate libquickjs-sys: must enable either the 'bundled' or the 'system' feature");
}

extern crate bindgen;

#[cfg(feature = "system")]
fn main() {
    #[cfg(not(feature = "bindgen"))]
    panic!("Invalid configuration for libquickjs-sys: Must either enable the bundled or the bindgen feature");

    let lib: std::borrow::Cow<str> = if let Ok(lib) = env::var("QUICKJS_LIBRARY_PATH") {
        lib.into()
    } else if cfg!(unix) {
        if exists(format!("/usr/lib/quickjs/{}.a", LIB_NAME)) {
            "/usr/lib/quickjs".into()
        } else if exists("/usr/local/lib/quickjs") {
            "/usr/local/lib/quickjs".into()
        } else {
            panic!("quickjs library could not be found. Try setting the QUICKJS_LIBRARY_PATH env variable");
        }
    } else {
        panic!("quickjs error: Windows is not supported yet");
    };

    // Instruct cargo to statically link quickjs.
    println!("cargo:rustc-link-search=native={}", lib);
    println!("cargo:rustc-link-lib=static={}", LIB_NAME);
}

#[cfg(feature = "bundled")]
fn main() {
    let src_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let quickjs_src_path = src_path.join("quickjs");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let binding = bindgen::builder()
        .header(src_path.join("wrapper.h").to_str().unwrap().to_string())
        .whitelist_function("(__)?(JS|js)_.*")
        .whitelist_var("JS_.*")
        .whitelist_type("JS.*")
        .generate()
        .unwrap();
    binding.write_to_file(out_path.join("bindings.rs")).unwrap();

    let quickjs_version = std::fs::read_to_string(quickjs_src_path.join("VERSION"))
        .expect("failed to read quickjs version");
    cc::Build::new()
        .files(
            [
                "cutils.c",
                "libbf.c",
                "libregexp.c",
                "libunicode.c",
                "quickjs.c",
                "quickjs-port.c",
            ]
            .iter()
            .map(|f| quickjs_src_path.join(f)),
        )
        .file(src_path.join("static-functions.c"))
        .define("_GNU_SOURCE", None)
        .define(
            "CONFIG_VERSION",
            format!("\"{}\"", quickjs_version.trim()).as_str(),
        )
        .define("CONFIG_BIGNUM", None)
        .flag_if_supported("/std:c11")
        // The below flags are used by the official Makefile.
        .flag_if_supported("-Wchar-subscripts")
        .flag_if_supported("-Wno-array-bounds")
        .flag_if_supported("-Wno-format-truncation")
        .flag_if_supported("-Wno-missing-field-initializers")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wundef")
        .flag_if_supported("-Wuninitialized")
        .flag_if_supported("-Wunused")
        .flag_if_supported("-Wwrite-strings")
        .flag_if_supported("-funsigned-char")
        // Below flags are added to supress warnings that appear on some
        // platforms.
        .flag_if_supported("-Wno-cast-function-type")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-enum-conversion")
        // cc uses the OPT_LEVEL env var by default, but we hardcode it to -O2
        // since release builds use -O3 which might be problematic for quickjs,
        // and debug builds only happen once anyway so the optimization slowdown
        // is fine.
        .opt_level(2)
        .compile(LIB_NAME);
}
