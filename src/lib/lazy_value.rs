use std::cell::UnsafeCell;
use std::clone::Clone;

/// Immutable store for a value that is created lazily using a closure.
/// The `get_ref()` method does not have a mutable reference to Self, therefore unsafe is used
/// Due to it's immutability, it is possible to return stable references to the contained value.
#[derive(Debug)]
pub struct LazyValue<T>(UnsafeCell<(bool, T)>);

impl<T> LazyValue<T> {
    pub fn new(initial_value: T) -> LazyValue<T> {
        LazyValue(UnsafeCell::new((false, initial_value)))
    }

    /// Resets the store to an uninitialized state, allowing the
    /// get_ref 'init_fn' to be executed again
    pub fn reset(&mut self) {
        unsafe {
            (*self.0.get()).0 = false;
        }
    }

    pub fn get_ref<F>(&self, init_fn: F) -> &T
    where
        F: FnOnce(&mut T),
    {
        let &mut (ref mut initialized, ref mut val) = unsafe { &mut *self.0.get() };
        if !*initialized {
            *initialized = true;
            init_fn(val);
        }
        &*val
    }
}

impl<T: Default> Default for LazyValue<T> {
    fn default() -> LazyValue<T> {
        LazyValue::new(T::default())
    }
}

impl<T> Clone for LazyValue<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let v = unsafe { (*self.0.get()).clone() };
        LazyValue(UnsafeCell::new(v))
    }
}
