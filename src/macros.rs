macro_rules! fail {
    ($e:expr) => {
        Err($e.into())
    };
    ($e:expr, $($arg:expr)+) => {
        Err(format!($e, $($arg)+,).into())
    };
}

macro_rules! try_opt {
    ($expr:expr) => {
        match $expr {
            Ok(item) => item,
            Err(e) => return Some(Err(std::convert::From::from(e))),
        }
    };
}

macro_rules! report {
    ($verbose:expr, $fmt:expr) => (
        if $verbose {
            eprintln!($fmt)
        }
    );
    ($verbose:expr, $fmt:expr, $($arg:tt)*) => (
        if $verbose {
            eprintln!($fmt, $($arg)*)
        }
    );
}
