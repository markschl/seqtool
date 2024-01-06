use std::any::Any;

pub trait AsAnyMut {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAnyMut for T {
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
