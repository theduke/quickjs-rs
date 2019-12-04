//! # Support for js modules.
//! Javascript files (.js, .jsm) are supported as modules.
//! Files must be referenced as relative paths to the path set with [`set_module_loader_with_path`].
//!
//! The QuickJS documentation clearly states that the bytecode that could be used for
//! binary module files is not stable.
//!
//! Implementation note: This is implemented via QuickJS-libc.
use crate::bindings::ContextWrapper;
use std::path::Path;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr::NonNull;
use std::io::ErrorKind;

use libquickjs_sys as q;

impl ContextWrapper{
    /// Sets a custom module loader.
    ///
    /// The loader will use the given path as root path to resolve files.
    /// In your scripts you must reference other files via relative paths.
    ///
    /// # Example
    /// `import {a_string} from './test.js';`
    pub fn set_module_loader_with_path(&mut self, path: &Path) {
        self.module_load_path = path.to_path_buf();

        let runtime_ptr = self.runtime.clone();
        let context_ptr = NonNull::from(self);
        unsafe {
            q::JS_SetModuleLoaderFunc(
                runtime_ptr,
                None,
                Some(jsc_module_loader),
                context_ptr.as_ptr() as *mut c_void,
            )
        }
    }
}

unsafe extern "C" fn jsc_module_loader(
    _ctx: *mut q::JSContext,
    module_name: *const c_char,
    opaque: *mut c_void,
) -> *mut q::JSModuleDef {
    use std::ffi::{CStr, OsStr};
    use std::os::unix::ffi::OsStrExt;
    // Safe: We can expect the C-API to hand us a valid module name c-string.
    let module_name = Path::new(OsStr::from_bytes(CStr::from_ptr(module_name).to_bytes()));

    let context_wrapper = opaque as *mut ContextWrapper;
    let context_wrapper = match NonNull::new(context_wrapper) {
        Some(v) => v,
        None => {
            eprintln!("load module failed: {:?}", module_name);
            return std::ptr::null_mut();
        }
    };
    let context_wrapper = context_wrapper.as_ref();
    let path = context_wrapper.module_load_path.join(module_name);
    println!("load module: {:?} from {:?}", module_name, path);

    let module_code = std::fs::read_to_string(path).and_then(|code| {
        context_wrapper
            .eval(&code, true, true)
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e.to_string()))
    });
    match module_code {
        Err(e) => {
            eprintln!("{}", e);
            return std::ptr::null_mut();
        }
        Ok(code) => {
            let p = code.value;
            q::js_module_set_import_meta(context_wrapper.context, p, 0, 0);

            // Note: code will go out of scope and the ref-count decreases.
            // Safe: const to mut cast. The engine runs in a single thread, as long as that is the case, the cast
            // is safe. We reuse a module if it is referenced multiple times in different script files and the internal
            // module "state" will be shared, which is absolutely fine.
            let p = p.u.ptr as *mut q::JSModuleDef;
            return p;
        }
    }
}

/*
// To avoid QuickJS-libc, module book keeping could be done manually

type ModuleInit = dyn Fn(*mut q::JSContext, *mut q::JSModuleDef);

thread_local! {
    static NATIVE_MODULE_INIT: RefCell<Option<Box<ModuleInit>>> = RefCell::new(None);
}

unsafe extern "C" fn native_module_init(
    ctx: *mut q::JSContext,
    m: *mut q::JSModuleDef,
) -> ::std::os::raw::c_int {
    NATIVE_MODULE_INIT.with(|init| {
        let init = init.replace(None).unwrap();
        init(ctx, m);
    });
    0
}
*/
