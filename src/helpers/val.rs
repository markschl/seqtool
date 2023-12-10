use std::ops::Deref;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TextValue(Vec<u8>);

impl TextValue {
    pub fn new() -> Self {
        Self(Vec::with_capacity(20))
    }

    pub fn clear(&mut self) -> &mut Vec<u8> {
        self.0.clear();
        &mut self.0
    }

    pub fn get_vec(&self) -> &Vec<u8> {
        &self.0
    }

    pub fn get_int(&self) -> Result<i64, String> {
        atoi::atoi(&self.0).ok_or_else(|| {
            format!(
                "Could not convert '{}' to decimal number.",
                String::from_utf8_lossy(&self.0)
            )
        })
    }

    pub fn get_float(&self) -> Result<f64, String> {
        // TODO: any more efficient way?
        std::str::from_utf8(&self.0)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                format!(
                    "Could not convert '{}' to integer.",
                    String::from_utf8_lossy(&self.0)
                )
            })
    }

    pub fn get_bool(&self) -> Result<bool, String> {
        match self.0.as_slice() {
            b"true" => Ok(true),
            b"false" => Ok(false),
            _ => Err(format!(
                "Could not convert '{}' to boolean (true/false).",
                String::from_utf8_lossy(&self.0)
            )),
        }
    }
}

impl Deref for TextValue {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
