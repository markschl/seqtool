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
}

pub trait ArgValue {
    fn from_str(val: &str) -> Option<Self>
    where
        Self: Sized;
}

impl ArgValue for String {
    fn from_str(val: &str) -> Option<Self> {
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
