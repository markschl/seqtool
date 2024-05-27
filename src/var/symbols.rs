use std::io::{self, Write};
use std::{cell::RefCell, fmt, str::Utf8Error};

use crate::helpers::{
    number::{parse_float, parse_int, Float, Interval},
    NA,
};
use crate::io::{Record, RecordAttr};

macro_rules! impl_value {
    ($t:ident ($inner_t:ty) {
            v: $inner:ty,
            $($field:ident: $ty:ty),*
        }
        [$self:ident, $record:ident]
        new => {
            v: $val_init:expr,
            $($field2:ident: $init_val:expr),*
        },
        get => $get:block,
        get_mut => $get_mut:block,
        bool => $bool:block,
        int => $int:block,
        float => $float:block,
        interval => $interval:block,
        $text_fn:ident => $text:block
    ) => {
        #[derive(Debug, Clone)]
        pub struct $t {
            v: $inner,
            $($field: $ty),*
        }

        impl Default for $t {
            fn default() -> Self {
                Self {
                    v: $val_init,
                    $($field2: $init_val),*
                }
            }
        }

        #[allow(dead_code)]
        impl $t {
            #[inline]
            pub fn get_mut(&mut $self) -> &mut $inner_t {
                $get_mut
            }

            #[inline]
            pub fn get(&$self) -> &$inner_t {
                $get
            }

            #[inline]
            pub fn get_bool(&$self, $record: &dyn Record) -> Result<bool, String> {
                $bool
            }

            #[inline]
            pub fn get_int(&$self, $record: &dyn Record) -> Result<i64, String> {
                $int
            }

            #[inline]
            pub fn get_float(&$self, $record: &dyn Record) -> Result<f64, String> {
                $float
            }

            #[inline]
            pub fn get_interval(&$self, $record: &dyn Record) -> Result<Interval, String> {
                $interval
            }

            #[inline]
            pub fn as_text<O>(&$self, $record: &dyn Record, $text_fn: impl FnOnce(&[u8]) -> O) -> O {
                $text
            }

            #[inline]
            pub fn as_str<O>(&$self, $record: &dyn Record, func: impl FnOnce(&str) -> O) -> Result<O, String> {
                $self.as_text($record, |t| {
                    let val = std::str::from_utf8(t).map_err(|e| e.to_string())?;
                    Ok(func(val))
                })
            }
        }
    };
}

impl_value!(
    BoolValue (bool) { v: bool, }
    [self, _record]
    new => { v: false, },
    get => { &self.v },
    get_mut => { &mut self.v },
    bool => { Ok(self.v) },
    int => { Ok(self.v as i64) },
    float => { Ok(self.v as i64 as f64) },
    interval => { Err(to_interval_err("boolean", self.v)) },
    text_fn => {
        text_fn(
            if self.v {
                &b"true"[..]
            } else {
                &b"false"[..]
            }
    )}
);

impl_value!(
    IntValue (i64) {
        v: i64,
        text: RefCell<Vec<u8>>
    }
    [self, _record]
    new => {
        v: 0,
        text: RefCell::new(Vec::with_capacity(20))
    },
    get => { &self.v },
    get_mut => {
        self.text.borrow_mut().clear();
        &mut self.v
    },
    bool => {
        match self.v {
            0 => Ok(false),
            _ => Ok(true),
        }
    },
    int => { Ok(self.v) },
    float => { Ok(self.v as f64) },
    interval => { Err(to_interval_err("integer number", self.v)) },
    text_fn => {
        let mut inner = self.text.borrow_mut();
        if inner.is_empty() {
            write!(inner, "{}", self.v).unwrap();
        }
        text_fn(&inner)
    }
);

impl_value!(
    FloatValue (f64) {
        v: Float,
        text: RefCell<Vec<u8>>
    }
    [self, _record]
    new => {
        v: Float::new(0.),
        text: RefCell::new(Vec::with_capacity(20))
    },
    get => { &self.v },
    get_mut => {
        self.text.borrow_mut().clear();
        &mut self.v
    },
    bool => {
        if self.v.inner() == 0. || self.v.is_nan() {
            Ok(false)
        } else {
            Ok(true)
        }
    },
    int => {
        if self.v.fract() == 0. {
            Ok(self.v.inner() as i64)
        } else {
            fail!(format!("Decimal number {} cannot be converted to integer", self.v))
        }
    },
    float => { Ok(self.v.inner()) },
    interval => { Err(to_interval_err("decimal number", self.v)) },
    text_fn => {
        let mut inner = self.text.borrow_mut();
        if inner.is_empty() {
            write!(inner, "{}", self.v).unwrap();
        }
        text_fn(&inner)
     }
);

impl_value!(
    FloatInterval (Interval) {
        v: Interval,
        text: RefCell<Vec<u8>>
    }
    [self, _record]
    new => {
        v: Interval::new(0., 0.),
        text: RefCell::new(Vec::with_capacity(20))
    },
    get => { &self.v },
    get_mut => {
        self.text.borrow_mut().clear();
        &mut self.v
    },
    bool => {
        Err(from_interval_err(self.v, "a boolean (true/false)"))
    },
    int => {
        Err(from_interval_err(self.v, "an integer number"))
    },
    float => {
        Err(from_interval_err(self.v, "a decimal number"))
    },
    interval => { Ok(self.v) },
    text_fn => {
        let mut inner = self.text.borrow_mut();
        if inner.is_empty() {
            write!(inner, "{}", self.v).unwrap();
        }
        text_fn(&inner)
     }
);

fn from_interval_err(val: Interval, what: &str) -> String {
    format!("Cannot convert the interval {} to {}", val, what)
}

fn to_interval_err<V: fmt::Display>(what: &str, value: V) -> String {
    format!("Cannot convert the {} '{}' to an interval", what, value)
}

impl_value!(
    TextValue (Vec<u8>) {
        v: Vec<u8>,
        // cache for float and integer values, so we don't need to re-calculate
        // at every access
        // we don't do this for booleans as it is simpler there
        float: RefCell<Option<f64>>,
        int: RefCell<Option<i64>>
    }
    [self, _record]
    new => {
        v: Vec::with_capacity(20),
        float: RefCell::new(None),
        int: RefCell::new(None)
    },
    get => { &self.v },
    get_mut => {
        self.int.take();
        self.float.take();
        self.v.clear();
        &mut self.v
    },
    bool => { parse_bool(&self.v) },
    int => {
        match self.int.borrow_mut().as_ref() {
            Some(i) => Ok(*i),
            None => parse_int(&self.v)
        }
     },
    float => {
        match self.float.borrow_mut().as_ref() {
            Some(f) => Ok(*f),
            None => parse_float(&self.v)
        }
    },
    interval => { unimplemented!() },
    text_fn => {
        text_fn(&self.v)
    }
);

impl SeqAttrValue {
    pub fn with_slice<O>(&self, record: &dyn Record, func: impl FnOnce(&[u8]) -> O) -> O {
        use RecordAttr::*;
        match self.v {
            Id => func(record.id()),
            Desc => func(record.desc().unwrap_or(b"")),
            Seq => func(&record.full_seq(&mut self.buffer.borrow_mut())),
        }
    }

    pub fn with_str<O>(
        &self,
        record: &dyn Record,
        func: impl FnOnce(&str) -> O,
    ) -> Result<O, Utf8Error> {
        self.with_slice(record, |s| std::str::from_utf8(s).map(func))
    }
}

impl_value!(
    SeqAttrValue (RecordAttr) {
        v: RecordAttr,
        buffer: RefCell<Vec<u8>>,
        // cache for float and integer values, so we don't need to re-calculate
        // at every access
        float: RefCell<Option<f64>>,
        int: RefCell<Option<i64>>
    }
    [self, record]
    new => {
        v: RecordAttr::Id,
        buffer: RefCell::new(Vec::with_capacity(100)),
        float: RefCell::new(None),
        int: RefCell::new(None)
    },
    get => { &self.v },
    get_mut => {
        self.int.take();
        self.float.take();
        &mut self.v
    },
    bool => { self.with_slice(record, parse_bool) },
    int => {
        match self.int.borrow_mut().as_ref() {
            Some(i) => Ok(*i),
            None => self.with_slice(record, parse_int)
        }
     },
    float => {
        match self.float.borrow_mut().as_ref() {
            Some(f) => Ok(*f),
            None => self.with_slice(record, parse_float)
        }
    },
    interval => {
        // avoid warning about record not being used
        let _ = record;
        unimplemented!()
    },
    text_fn => {
        self.with_slice(record, text_fn)
    }
);

fn parse_bool(s: &[u8]) -> Result<bool, String> {
    match s {
        b"true" => Ok(true),
        b"false" => Ok(false),
        _ => {
            if let Ok(f) = parse_float(s) {
                Ok(f != 0.)
            } else {
                Err(format!(
                    "Could not convert '{}' to boolean (true/false).",
                    String::from_utf8_lossy(s)
                ))
            }
        }
    }
}

/// Variable value enum, optimized for keeping a constant type.
/// TextValue/IntValue/FloatValue therefore can be set to None
/// (expecting that they can be set back to Some(value) of the same type),
/// thus retaining allocations of text vectors.
#[derive(Debug, Clone)]
pub enum Value {
    Text(TextValue),
    Int(IntValue),
    Float(FloatValue),
    Interval(FloatInterval),
    Bool(BoolValue),
    Attr(SeqAttrValue),
}

macro_rules! mut_accessor {
    ($fn_name:ident, $variant:ident, $t:ty) => {
        #[inline]
        pub fn $fn_name(&mut self) -> &mut $t {
            loop {
                match self {
                    Value::$variant(ref mut v) => return v.get_mut(),
                    _ => {
                        *self = Value::$variant(Default::default());
                    }
                }
            }
        }
    };
}

macro_rules! impl_set {
    ($fn_name:ident, $get_fn:ident, $t:ty) => {
        #[inline]
        pub fn $fn_name(&mut self, value: $t) {
            *self.$get_fn() = value;
        }
    };
}

macro_rules! accessor {
    ($fn_name:ident, $t:ty) => {
        #[inline]
        pub fn $fn_name(&self, record: &dyn Record) -> Result<$t, String> {
            use Value::*;
            match self {
                Text(ref v) => v.$fn_name(record),
                Int(ref v) => v.$fn_name(record),
                Float(ref v) => v.$fn_name(record),
                Interval(ref v) => v.$fn_name(record),
                Bool(ref v) => v.$fn_name(record),
                Attr(ref v) => v.$fn_name(record),
            }
        }
    };
}

impl Value {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Float(_) | Value::Bool(_))
    }

    mut_accessor!(mut_bool, Bool, bool);
    mut_accessor!(mut_int, Int, i64);
    mut_accessor!(mut_float, Float, f64);
    mut_accessor!(mut_interval, Interval, Interval);
    mut_accessor!(mut_text, Text, Vec<u8>);
    mut_accessor!(mut_attr, Attr, RecordAttr);
    impl_set!(set_bool, mut_bool, bool);
    impl_set!(set_int, mut_int, i64);
    impl_set!(set_float, mut_float, f64);
    impl_set!(set_interval, mut_interval, Interval);
    impl_set!(set_attr, mut_attr, RecordAttr);

    #[inline]
    pub fn set_text(&mut self, value: &[u8]) {
        let text = self.mut_text();
        text.clear();
        text.extend_from_slice(value);
    }

    // accessor!(get_bool, bool);
    accessor!(get_int, i64);
    accessor!(get_float, f64);
    accessor!(get_interval, Interval);

    #[inline]
    pub fn as_text<E>(
        &self,
        record: &dyn Record,
        func: impl FnOnce(&[u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        use Value::*;
        match self {
            Text(ref v) => v.as_text(record, func),
            Int(ref v) => v.as_text(record, func),
            Float(ref v) => v.as_text(record, func),
            Interval(ref v) => v.as_text(record, func),
            Bool(ref v) => v.as_text(record, func),
            Attr(ref v) => v.as_text(record, func),
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Bool(BoolValue::default())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Value::*;
        match self {
            Text(v) => write!(f, "{}", String::from_utf8_lossy(&v.v))?,
            Int(v) => write!(f, "{}", v.v)?,
            Float(v) => write!(f, "{}", v.v)?,
            Interval(v) => write!(f, "{}", v.v)?,
            Bool(v) => write!(f, "{}", v.v)?,
            Attr(a) => write!(f, "{}", a.v)?,
        }
        Ok(())
    }
}

/// This type caches Value instances, allowing them to be
/// set to None and back to Some(Value) without losing allocations.
#[derive(Debug, Clone, Default)]
pub struct OptValue {
    value: Value,
    is_some: bool,
}

impl OptValue {
    pub fn inner(&self) -> Option<&Value> {
        if self.is_some {
            Some(&self.value)
        } else {
            None
        }
    }

    pub fn inner_mut(&mut self) -> &mut Value {
        self.is_some = true;
        &mut self.value
    }

    pub fn set_none(&mut self) {
        self.is_some = false;
    }

    pub fn to_text<W: io::Write + ?Sized>(
        &self,
        record: &dyn Record,
        out: &mut W,
    ) -> io::Result<()> {
        if let Some(v) = self.inner() {
            v.as_text(record, |s| out.write_all(s))
        } else {
            out.write_all(NA)
        }
    }
}

impl fmt::Display for OptValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_some {
            write!(f, "{}", self.value)
        } else {
            write!(f, "N/A")
        }
    }
}

/// Simple symbol table for storing values of different types,
/// serving as the intermediate value store for all variables
/// and expressions.
// TODO: nicer API: set_int(Some(x)) or set_int(None), set_float(...), etc. (but quite wordy...)
// TODO: null / undefined are only interpreted as text, which is not consistent with JS
#[derive(Debug, Clone, Default)]
pub struct SymbolTable(Vec<OptValue>);

impl SymbolTable {
    #[inline]
    pub fn new(size: usize) -> SymbolTable {
        //use std::mem::size_of; println!("{}", size_of::<Value>());
        SymbolTable(vec![OptValue::default(); size])
    }

    pub fn resize(&mut self, size: usize) {
        if size > self.0.len() {
            for _ in self.0.len()..size {
                self.0.push(OptValue::default());
            }
        } else {
            self.0.truncate(size);
        }
    }

    #[inline]
    pub fn get(&self, id: usize) -> &OptValue {
        self.0.get(id).unwrap()
    }

    #[inline]
    pub fn get_mut(&mut self, id: usize) -> &mut OptValue {
        self.0.get_mut(id).unwrap()
    }
}
