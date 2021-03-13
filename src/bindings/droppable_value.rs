/// A small wrapper that frees resources that have to be freed
/// automatically when they go out of scope.
pub struct DroppableValue<T, F>
where
    F: FnMut(&mut T),
{
    value: T,
    drop_fn: F,
}

impl<T, F> DroppableValue<T, F>
where
    F: FnMut(&mut T),
{
    pub fn new(value: T, drop_fn: F) -> Self {
        Self { value, drop_fn }
    }
}

impl<T, F> Drop for DroppableValue<T, F>
where
    F: FnMut(&mut T),
{
    fn drop(&mut self) {
        (self.drop_fn)(&mut self.value);
    }
}

impl<T, F> std::ops::Deref for DroppableValue<T, F>
where
    F: FnMut(&mut T),
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T, F> std::ops::DerefMut for DroppableValue<T, F>
where
    F: FnMut(&mut T),
{
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
