use std::str::FromStr;
use std::io::Write;

use lib::lazy_value::LazyValue;
use error::CliResult;

#[derive(Debug, Clone)]
enum Value {
    None,
    Text(Vec<u8>, LazyValue<Result<Option<i64>, ()>>, LazyValue<Result<Option<f64>, ()>>),
    Int(i64, LazyValue<Vec<u8>>),
    Float(f64, LazyValue<Vec<u8>>),
}

#[derive(Debug, Clone)]
struct ParseNumError;

/// Simple symbol table for storing values of different types. They can be converted
/// to one another. String -> numeric parsing results are cached for repeated retrieval.
#[derive(Debug, Clone)]
pub struct Table(Vec<Value>);

#[allow(dead_code)]
impl Table {
    #[inline]
    pub fn new(size: usize) -> Table {
        //use std::mem::size_of; println!("{}", size_of::<Value>());
        Table(vec![Value::None; size])
    }

    pub fn resize(&mut self, size: usize) {
        if size > self.0.len() {
            for _ in self.0.len()..size {
                self.0.push(Value::None);
            }
        } else {
            self.0.truncate(size);
        }
    }

    #[inline]
    pub fn set_text(&mut self, id: usize, text: &[u8]) {
        self.mut_text(id).extend_from_slice(text)
    }

    #[inline]
    pub fn mut_text(&mut self, id: usize) -> &mut Vec<u8> {
        let v = &mut self.0[id];
        match *v {
            Value::Text(..) => {}
            _ => *v = Value::Text(vec![], LazyValue::new(Ok(Some(0))), LazyValue::new(Ok(Some(0.)))),
        }
        if let Value::Text(ref mut s, ref mut i, ref mut f) = *v {
            i.reset();
            f.reset();
            s.clear();
            s
        } else {
            unreachable!();
        }
    }

    #[inline]
    pub fn set_int(&mut self, id: usize, val: i64) {
        let v = &mut self.0[id];
        if let Value::Int(ref mut intval, ref mut strval) = *v {
            *intval = val;
            strval.reset();
        } else {
            *v = Value::Int(val, LazyValue::default());
        }
    }

    #[inline]
    pub fn set_float(&mut self, id: usize, val: f64) {
        let v = &mut self.0[id];
        if let Value::Float(ref mut intval, ref mut strval) = *v {
            *intval = val;
            strval.reset();
        } else {
            *v = Value::Float(val, LazyValue::default());
        }
    }

    #[inline]
    pub fn set_none(&mut self, id: usize) {
        self.set(id, Value::None)
    }

    #[inline]
    pub fn get_text(&self, id: usize) -> Option<&[u8]> {
        match *self.get(id) {
            Value::Text(ref s, ..) => Some(s),
            Value::Int(i, ref s) => Some(s.get_ref(|s| {
                s.clear();
                write!(s, "{}", i).unwrap();
            })),
            Value::Float(f, ref s) => Some(s.get_ref(|s| {
                s.clear();
                write!(s, "{}", f).unwrap();
            })),
            Value::None => None,
        }
    }

    #[inline]
    pub fn get_int(&self, id: usize) -> CliResult<Option<i64>> {
        match *self.get(id) {
            Value::Int(i, _) =>
                Ok(Some(i)),
            Value::Float(f, _) =>
                Ok(Some(f as i64)),
            Value::Text(ref s, ref v, _) =>
                get_num(s, v).map_err(|_|
                    format!("Could not parse '{}' as integer.", String::from_utf8_lossy(s)).into()
                ),
            Value::None =>
                Ok(None),
        }
    }

    #[inline]
    pub fn get_float(&self, id: usize) -> CliResult<Option<f64>> {
        match *self.get(id) {
            Value::Float(f, _) =>
                Ok(Some(f)),
            Value::Int(i, _) =>
                Ok(Some(i as f64)),
            Value::Text(ref s, _, ref v) =>
                get_num(s, v).map_err(|_|
                    format!("Could not parse '{}' as float.", String::from_utf8_lossy(s)).into()
                ),
            Value::None =>
                Ok(None),
        }
    }

    pub fn is_none(&self, id: usize) -> bool {
        match *self.get(id) {
            Value::None => true,
            _ => false
        }
    }

    pub fn is_empty(&self, id: usize) -> bool {
        match *self.get(id) {
            Value::Text(ref s, _, _) => s.is_empty(),
            Value::None => true,
            _ => false
        }
    }

    #[inline]
    fn get(&self, id: usize) -> &Value {
        if let Some(v) = self.0.get(id) {
            v
        } else {
            panic!(format!(
                "Attempt to retrieve symbol {}, but table size is {}. This is a bug!",
                id,
                self.0.len()
            ));
        }
    }

    #[inline]
    fn set(&mut self, id: usize, val: Value) {
        if id < self.0.len() {
            self.0[id] = val;
        } else {
            panic!(format!(
                "Attempt to set value {:?} at index {}, but table size is {}. This is a bug!",
                val,
                id,
                self.0.len()
            ));
        }
    }
}

#[inline]
fn get_num<T: Copy + FromStr>(s: &[u8], value: &LazyValue<Result<Option<T>, ()>>) -> Result<Option<T>, ()> {
    // TODO: "unnecessary" utf8 conversion since there is no function for parsing numbers from &[u8]
    *value.get_ref(|v| {
        if s.is_empty() {
            *v = Ok(None);
        } else  {
            *v = ::std::str::from_utf8(s)
                .map_err(|_| ())
                .and_then(|s| s.parse().map_err(|_| ()).map(Some));
        }
    })
}
