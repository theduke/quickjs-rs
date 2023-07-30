use libquickjs_sys::JSContext;

pub struct SerializeMap<'a> {
    context: &'a mut JSContext,
}
