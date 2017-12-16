/// This trait works around the problem of a Result being
/// "trapped" within an Option by providing the `map_res()`
/// function, which works like `.map()`, but extracts
/// the result returned by the closure.
pub trait MapRes<T, U, E, F>
where
    F: FnOnce(&T) -> Result<U, E>,
{
    fn map_res(&self, func: F) -> Result<Option<U>, E>;
}

impl<T, U, E, F> MapRes<T, U, E, F> for Option<T>
where
    F: FnOnce(&T) -> Result<U, E>,
{
    #[inline]
    fn map_res(&self, func: F) -> Result<Option<U>, E> {
        if let Some(inner) = self.as_ref() {
            match func(inner) {
                Ok(res) => Ok(Some(res)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }
}
