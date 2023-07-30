use libquickjs_sys::JSContext;

pub struct Context {
    context: *const JSContext,
    should_drop: bool,
}

impl Context {
    pub fn new(context: *mut JSContext) -> Self {
        Self {
            context,
            should_drop: true,
        }
    }

    pub fn new_without_drop(context: *mut JSContext) -> Self {
        Self {
            context,
            should_drop: false,
        }
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut JSContext {
        self.context as *mut _
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if self.should_drop {
            unsafe { libquickjs_sys::JS_FreeContext(self.context as *mut _) };
        }
    }
}
