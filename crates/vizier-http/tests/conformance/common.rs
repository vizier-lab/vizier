use serde_json::Value;

pub struct Vector {
    pub method: &'static str,
    pub path: &'static str,
    pub body: Option<Value>,
}

pub fn v(method: &'static str, path: &'static str, body: Option<Value>) -> Vector {
    Vector { method, path, body }
}
