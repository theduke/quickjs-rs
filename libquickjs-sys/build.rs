use std::path::{Path, PathBuf};

use std::env;

fn exists(path: impl AsRef<Path>) -> bool {
    PathBuf::from(path.as_ref()).exists()
}

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
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let embed_dir = crate_dir.join("embed");
    let quickjs_dir = embed_dir.join("quickjs");

    copy_dir::copy_dir(quickjs_dir, &code_dir).expect("Could not copy quickjs directory");

    #[cfg(feature = "patched")]
    apply_patches(&code_dir);

    eprintln!("Compiling quickjs...");
    std::process::Command::new("make")
        .arg("libquickjs.a")
        .current_dir(&code_dir)
        .spawn()
        .expect("Could not compile quickjs")
        .wait()
        .expect("Could not compile quickjs");

    let bindings_rs = embed_dir.join("bindings.rs");
    std::fs::copy(bindings_rs, out_path.join("bindings.rs")).expect("Could not copy bindings.rs");

    if cfg!(windows) {
        for res in std::fs::read_dir(&code_dir).unwrap() {
            eprintln!("{}", res.unwrap().path().display());
        }
        std::fs::copy(
            code_dir.join("libquickjs.a"),
            code_dir.join("libquickjs.lib"),
        )
        .expect("Could not move static library");
    }

    // Instruct cargo to statically link quickjs.
    println!("cargo:rustc-link-lib=static=quickjs");
    println!("cargo:rustc-link-search=native={}", code_dir.display(),);
}

#[cfg(feature = "patched")]
fn apply_patches(code_dir: &PathBuf) {
    use std::fs;

    eprintln!("Applying patches...");
    for patch in fs::read_dir("./embed/patches").expect("Could not open patches directory") {
        let patch = patch.expect("Could not open patch");
        eprintln!("Applying {:?}...", patch.file_name());
        let status = std::process::Command::new("patch")
            .current_dir(&code_dir)
            .arg("-i")
            .arg(fs::canonicalize(patch.path()).expect("Cannot canonicalize patch path"))
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
