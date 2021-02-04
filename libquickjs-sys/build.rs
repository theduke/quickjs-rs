use std::path::{Path, PathBuf};

use std::env;

fn exists(path: impl AsRef<Path>) -> bool {
    PathBuf::from(path.as_ref()).exists()
}

const LIB_NAME: &str = "quickjs";

#[cfg(all(not(feature = "system"), not(feature = "bundled")))]
fn main() {
    panic!("Invalid config for crate libquickjs-sys: must enable either the 'bundled' or the 'system' feature");
}

#[cfg(feature = "system")]
extern crate bindgen;

#[cfg(feature = "system")]
fn main() {
    #[cfg(not(feature = "bindgen"))]
    panic!("Invalid configuration for libquickjs-sys: Must either enable the bundled or the bindgen feature");

    #[cfg(feature = "patched")]
    panic!("Invalid configuration for libquickjs-sys: the patched feature is incompatible with the system feature");

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
    let embed_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let code_dir = out_path.join("quickjs");
    if exists(&code_dir) {
        std::fs::remove_dir_all(&code_dir).unwrap();
    }
    copy_dir::copy_dir(embed_path.join("quickjs"), &code_dir)
        .expect("Could not copy quickjs directory");

    #[cfg(feature = "patched")]
    apply_patches(&code_dir);

    std::fs::copy(
        embed_path.join("static-functions.c"),
        code_dir.join("static-functions.c"),
    )
    .expect("Could not copy static-functions.c");

    eprintln!("Compiling quickjs...");
    let quickjs_version =
        std::fs::read_to_string(code_dir.join("VERSION")).expect("failed to read quickjs version");
    cc::Build::new()
        .files(
            [
                "cutils.c",
                "libbf.c",
                "libregexp.c",
                "libunicode.c",
                "quickjs.c",
                // Custom wrappers.
                "static-functions.c",
            ]
            .iter()
            .map(|f| code_dir.join(f)),
        )
        .define("_GNU_SOURCE", None)
        .define(
            "CONFIG_VERSION",
            format!("\"{}\"", quickjs_version.trim()).as_str(),
        )
        .define("CONFIG_BIGNUM", None)
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

    std::fs::copy(embed_path.join("bindings.rs"), out_path.join("bindings.rs"))
        .expect("Could not copy bindings.rs");
}

#[cfg(feature = "patched")]
fn apply_patches(code_dir: &PathBuf) {
    use std::fs;

    eprintln!("Applying patches...");
    let embed_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("embed");
    let patches_path = embed_path.join("patches");
    for patch in fs::read_dir(patches_path).expect("Could not open patches directory") {
        let patch = patch.expect("Could not open patch");
        eprintln!("Applying {:?}...", patch.file_name());
        let status = std::process::Command::new("patch")
            .current_dir(&code_dir)
            .arg("-i")
            .arg(patch.path())
            .spawn()
            .expect("Could not apply patches")
            .wait()
            .expect("Could not apply patches");
        assert!(
            status.success(),
            "Patch command returned non-zero exit code"
        );
    }
}
