use std::any::Any;

// pub trait AsAny {
//     fn as_any(&self) -> &dyn Any;
// }

// impl<T: Any> AsAny for T {
//     fn as_any(&self) -> &dyn Any {
//         self
//     }
// }

pub trait AsAnyMut {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAnyMut for T {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
