use std::cell::RefCell;
use std::thread::LocalKey;

pub fn with_mut_thread_local<I, F, T, R>(
    lkey: &'static LocalKey<RefCell<Option<T>>>,
    init: I,
    f: F,
) -> R
where
    F: FnOnce(&mut T) -> R,
    I: FnOnce() -> T,
{
    lkey.with(|d| {
        let mut d = d.borrow_mut();
        let data = if let Some(ref mut data) = *d {
            data
        } else {
            *d = Some(init());
            d.as_mut().unwrap()
        };
        f(data)
    })
}
