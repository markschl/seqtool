#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct Func {
    pub name: String,
    // there can be a maximum of 4 args
    pub args: Vec<String>,
}

impl Func {
    pub fn new(name: &str, args: &[String]) -> Self {
        Self {
            name: name.to_string(),
            args: args.to_vec(),
        }
    }

    // pub fn var(name: String) -> Self {
    //     Self::with_args(name, Default::default())
    // }

    pub fn expr(expr: &str) -> Self {
        Self::with_args("____expr".to_string(), vec![expr.to_string()])
    }

    pub fn with_args(name: String, args: Vec<String>) -> Self {
        Self { name, args }
    }

    pub fn num_args(&self) -> usize {
        self.args.len()
    }

    // pub fn ensure_num_args(&self, num_args: usize) -> Result<(), String> {
    //     self.ensure_arg_range(num_args, num_args)
    // }

    // pub fn ensure_arg_range(&self, min_args: usize, max_args: usize) -> Result<(), String> {
    //     let n = self.num_args();
    //     // if n == 0 && max_args > 0 {
    //     //     return Err(format!("'{}' is not a function with arguments, but a simple variable", self.name));
    //     // }
    //     let what = if n < min_args {
    //         "Not enough"
    //     } else if n > max_args {
    //         "Too many"
    //     } else {
    //         return Ok(());
    //     };
    //     Err(format!(
    //         "{} arguments provided to function '{}', expected {} but found {}.",
    //         what,
    //         self.name,
    //         if min_args != max_args {
    //             format!("{}-{}", min_args, max_args)
    //         } else {
    //             min_args.to_string()
    //         },
    //         n
    //     ))
    // }

    // pub fn ensure_no_args(&self) -> Result<(), String> {
    //     self.ensure_num_args(0)
    // }

    // pub fn one_arg(&self) -> Result<&str, String> {
    //     self.ensure_num_args(1)?;
    //     Ok(&self.args[0].as_ref())
    // }

    // pub fn one_arg_as<T: ArgValue>(&self) -> Result<T, String> {
    //     self.ensure_num_args(1)?;
    //     self.arg_as(0).unwrap()
    // }
    pub fn opt_arg(&self, num: usize) -> Option<&str> {
        self.args.get(num).map(|s| s.as_str())
    }

    pub fn arg(&self, num: usize) -> &str {
        &self.args[num]
    }

    pub fn arg_as<T: ArgValue>(&self, num: usize) -> Result<T, String> {
        self.opt_arg_as(num).unwrap()
    }

    pub fn opt_arg_as<T: ArgValue>(&self, num: usize) -> Option<Result<T, String>> {
        self.args.get(num).map(|a| {
            ArgValue::from_str(a)
                .ok_or_else(|| format!("Invalid argument for {}: {}", self.name, a))
        })
    }

    // pub fn str_arg_or_empty(&self, num: usize) -> Result<String, String> {
    //     self.arg_as(num).unwrap_or_else(|| Ok("".to_string()))
    // }
}

pub trait ArgValue {
    fn from_str(val: &str) -> Option<Self>
    where
        Self: Sized;
}

impl ArgValue for String {
    fn from_str(val: &str) -> Option<Self> {
        // if let Some(&c0) = val.as_bytes().first() {
        //     if c0 == b'"' || c0 == b'\'' {
        //         let c1 = *val.as_bytes().last().unwrap();
        //         if c0 != c1 {
        //             return None;
        //         }
        //         return Some(val[1..val.len() - 1].to_string());
        //     }
        //     // TODO: we currently allow non-quoted string arguments
        //     // (not valid javascript)
        //     return Some(val.to_string());
        // }
        // None
        Some(val.to_string())
    }
}

impl ArgValue for i64 {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for usize {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for f64 {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}

impl ArgValue for bool {
    fn from_str(val: &str) -> Option<Self> {
        val.parse().ok()
    }
}
