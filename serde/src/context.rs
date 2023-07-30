use libquickjs_sys::JSContext;

pub struct Context {
    context: *const JSContext,
}

impl Context {
    pub fn new(context: *mut JSContext) -> Self {
        Self { context }
    }

    pub(crate) fn as_ptr(&self) -> *const JSContext {
        self.context
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut JSContext {
        self.context as *mut _
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { libquickjs_sys::JS_FreeContext(self.context as *mut _) };
    }
}
