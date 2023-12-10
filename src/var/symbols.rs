use std::{cell::RefCell, fmt, io::Write, str::Utf8Error};

use crate::io::{Record, SeqAttr};

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
    new => {
        v: false,
    },
    get => { &self.v },
    get_mut => { &mut self.v },
    bool => { Ok(self.v) },
    int => { Ok(self.v as i64) },
    float => { Ok(self.v as i64 as f64) },
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
            1 => Ok(true),
            _ => Err(format!("Cannot convert {} to boolean", self.v)),
        }
    },
    int => { Ok(self.v) },
    float => { Ok(self.v as f64) },
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
        v: f64,
        text: RefCell<Vec<u8>>
    }
    [self, _record]
    new => {
        v: 0.,
        text: RefCell::new(Vec::with_capacity(20))
    },
    get => { &self.v },
    get_mut => {
        self.text.borrow_mut().clear();
        &mut self.v
    },
    bool => {
        if self.v == 0. {
            Ok(false)
        } else if self.v == 1. {
            Ok(true)
        } else {
            Err(format!("Cannot convert {} to boolean", self.v))
        }
    },
    int => {
        if self.v.fract() == 0. {
            Ok(self.v as i64)
        } else {
            fail!(format!("Decimal number {} cannot be converted to integer", self.v))
        }
    },
    float => { Ok(self.v) },
    text_fn => {
        let mut inner = self.text.borrow_mut();
        if inner.is_empty() {
            write!(inner, "{}", self.v).unwrap();
        }
        text_fn(&inner)
     }
);

impl_value!(
    TextValue (Vec<u8>) {
        v: crate::helpers::val::TextValue,
        // cache for float and integer values, so we don't need to re-calculate
        // at every access
        float: RefCell<Option<f64>>,
        int: RefCell<Option<i64>>
    }
    [self, _record]
    new => {
        v: crate::helpers::val::TextValue::new(),
        float: RefCell::new(None),
        int: RefCell::new(None)
    },
    get => { self.v.get_vec() },
    get_mut => {
        self.int.take();
        self.float.take();
        self.v.clear()
    },
    bool => { self.v.get_bool() },
    int => {
        match self.int.borrow_mut().as_ref() {
            Some(i) => Ok(*i),
            None => self.v.get_int()
        }
     },
    float => {
        match self.float.borrow_mut().as_ref() {
            Some(f) => Ok(*f),
            None => self.v.get_float()
        }
    },
    text_fn => {
        text_fn(&self.v)
    }
);

impl SeqAttrValue {
    pub fn with_slice<O>(&self, record: &dyn Record, func: impl FnOnce(&[u8]) -> O) -> O {
        match self.v {
            SeqAttr::Id => func(record.id_bytes()),
            SeqAttr::Desc => func(record.desc_bytes().unwrap_or(b"")),
            SeqAttr::Seq => func(&record.full_seq(&mut self.buffer.borrow_mut())),
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
    SeqAttrValue (SeqAttr) {
        v: SeqAttr,
        buffer: RefCell<Vec<u8>>,
        // cache for float and integer values, so we don't need to re-calculate
        // at every access
        float: RefCell<Option<f64>>,
        int: RefCell<Option<i64>>
    }
    [self, record]
    new => {
        v: SeqAttr::Id,
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
    bool => {
        self.with_slice(record, |s| {
            match s {
                b"true" => Ok(true),
                b"false" => Ok(false),
                _ => Err(format!(
                    "Could not convert '{}' to boolean (true/false).",
                    String::from_utf8_lossy(s)))
            }
        })
    },
    int => {
        match self.int.borrow_mut().as_ref() {
            Some(i) => Ok(*i),
            None => self.with_slice(record, |s| {
                atoi::atoi(s)
                    .ok_or_else(|| format!(
                        "Could not convert '{}' to integer.",
                        String::from_utf8_lossy(s)))
            })
        }
     },
    float => {
        match self.float.borrow_mut().as_ref() {
            Some(f) => Ok(*f),
            None => self.with_slice(record, |s| {
                std::str::from_utf8(s)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| format!(
                        "Could not convert '{}' to decimal number.",
                        String::from_utf8_lossy(s)))
            })
        }
    },
    text_fn => {
        self.with_slice(record, text_fn)
    }
);

/// Variable value enum, optimized for keeping a constant type.
/// TextValue/IntValue/FloatValue therefore can be set to None
/// (expecting that they can be set back to Some(value) of the same type),
/// thus retaining allocations of text vectors.
#[derive(Debug, Clone)]
pub enum Value {
    Text(TextValue),
    Int(IntValue),
    Float(FloatValue),
    Bool(BoolValue),
    Attr(SeqAttrValue),
}

impl Default for Value {
    fn default() -> Self {
        Value::Bool(BoolValue::default())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Text(v) => write!(f, "{}", String::from_utf8_lossy(&v.v))?,
            Value::Int(v) => write!(f, "{}", v.v)?,
            Value::Float(v) => write!(f, "{}", v.v)?,
            Value::Bool(v) => write!(f, "{}", v.v)?,
            Value::Attr(a) => write!(f, "{}", a.v)?,
        }
        Ok(())
    }
}

macro_rules! mut_accessor {
    ($fn_name:ident, $variant:ident, $t:ty) => {
        #[inline]
        pub fn $fn_name(&mut self) -> &mut $t {
            self.is_none = false;
            loop {
                match self.value {
                    Value::$variant(ref mut v) => return v.get_mut(),
                    _ => {
                        self.value = Value::$variant(Default::default());
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
        pub fn $fn_name(&self, record: &dyn Record) -> Option<Result<$t, String>> {
            if !self.is_none {
                Some(match self.value {
                    Value::Text(ref v) => v.$fn_name(record),
                    Value::Int(ref v) => v.$fn_name(record),
                    Value::Float(ref v) => v.$fn_name(record),
                    Value::Bool(ref v) => v.$fn_name(record),
                    Value::Attr(ref v) => v.$fn_name(record),
                })
            } else {
                None
            }
        }
    };
}

/// This type caches Value instances, allowing them to be
/// set to None and back to Some(Value) without losing allocations.
#[derive(Debug, Clone, Default)]
pub struct OptValue {
    value: Value,
    is_none: bool,
}

#[allow(dead_code)]
impl OptValue {
    pub fn value(&self) -> Option<&Value> {
        if !self.is_none {
            Some(&self.value)
        } else {
            None
        }
    }

    mut_accessor!(mut_bool, Bool, bool);
    mut_accessor!(mut_int, Int, i64);
    mut_accessor!(mut_float, Float, f64);
    mut_accessor!(mut_text, Text, Vec<u8>);
    mut_accessor!(mut_attr, Attr, SeqAttr);
    impl_set!(set_bool, mut_bool, bool);
    impl_set!(set_int, mut_int, i64);
    impl_set!(set_float, mut_float, f64);
    impl_set!(set_attr, mut_attr, SeqAttr);

    #[inline]
    pub fn set_text(&mut self, value: &[u8]) {
        let text = self.mut_text();
        text.clear();
        text.extend_from_slice(value);
    }

    #[inline]
    pub fn set_none(&mut self) {
        self.is_none = true;
    }

    accessor!(get_bool, bool);
    accessor!(get_int, i64);
    accessor!(get_float, f64);

    #[inline]
    pub fn as_text(&self, record: &dyn Record, func: impl FnOnce(&[u8])) -> bool {
        if !self.is_none {
            match self.value {
                Value::Text(ref v) => v.as_text(record, func),
                Value::Int(ref v) => v.as_text(record, func),
                Value::Float(ref v) => v.as_text(record, func),
                Value::Bool(ref v) => v.as_text(record, func),
                Value::Attr(ref v) => v.as_text(record, func),
            }
        }
        !self.is_none
    }
}

impl fmt::Display for OptValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_none {
            write!(f, "{}", self.value)
        } else {
            write!(f, "undefined")
        }
    }
}

/// Simple symbol table for storing values of different types,
/// serving as the intermediate value store for all variables
/// and expressions.
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
        self.0.get(id).expect("Bug: invalid symbol id")
    }

    #[inline]
    pub fn get_mut(&mut self, id: usize) -> &mut OptValue {
        self.0.get_mut(id).expect("Bug: invalid symbol id")
    }
}
