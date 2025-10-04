//! Functions related to constructing a variable/function enum type with its arguments.

use std::fmt::Display;

pub trait FromArg<A>: Sized {
    fn from_arg(func_name: &str, arg_name: &str, arg: A) -> Result<Self, String>;
}

impl<'a> FromArg<&'a str> for &'a str {
    fn from_arg(_: &str, _: &str, value: &'a str) -> Result<Self, String> {
        Ok(value)
    }
}

macro_rules! impl_from_arg {
    ($ty:ty, $cnv:expr, $what:expr) => {
        impl FromArg<&str> for $ty {
            fn from_arg(func_name: &str, arg_name: &str, value: &str) -> Result<Self, String> {
                $cnv(value).map_err(|_| invalid_value(func_name, arg_name, value))
            }
        }
    };
}

impl_from_arg!(usize, |s: &str| s.parse(), "an integer number");
impl_from_arg!(f64, |s: &str| s.parse(), "a decimal number");
impl_from_arg!(bool, |s: &str| s.parse(), "a boolean (true/false)");
impl_from_arg!(String, |s: &str| Ok::<_, String>(s.to_string()), "a string");

#[inline(never)]
pub fn invalid_value<V: Display>(var_name: &str, arg_name: &str, value: V) -> String {
    format!("Invalid value for argument '{arg_name}' of function '{var_name}': '{value}'")
}

#[inline(never)]
pub fn missing_argument(var_name: &str, arg_name: &str) -> String {
    format!("The function '{var_name}' is missing the argument '{arg_name}'")
}

#[inline(never)]
pub fn too_many_args<V: Display>(var_name: &str, max_args: usize, arg: V) -> String {
    format!(
        "The function '{}' got an unexpected argument '{}', expecting only {} argument{}",
        var_name,
        arg,
        max_args,
        if max_args == 1 { "" } else { "s" }
    )
}
