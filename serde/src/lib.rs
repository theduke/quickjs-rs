mod context;
mod errors;
mod ser;

pub use context::Context;
use libquickjs_sys::JSValue;
use serde::Serialize;

pub fn serialize<T: ?Sized>(
    value: &T,
    context: &mut Context,
) -> Result<JSValue, errors::SerializationError>
where
    T: Serialize,
{
    let serializer = ser::Serializer::new(context);

    value.serialize(serializer)
}
