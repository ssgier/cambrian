use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::value::Number;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Value(pub Node);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Node {
    Real(f64),
    Int(i64),
    Bool(bool),
    Sub(HashMap<String, Box<Node>>),
    AnonMap(HashMap<usize, Box<Node>>),
    Variant(String, Box<Node>),
    Enum(String),
    Optional(Option<Box<Node>>),
    Const,
}

impl Value {
    pub fn to_json(&self) -> serde_json::Value {
        self.0.to_json()
    }
}

impl Node {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Node::Real(number) => serde_json::Value::Number(Number::from_f64(*number).unwrap()),
            Node::Int(number) => serde_json::Value::Number(Number::from(*number)),
            Node::Bool(val) => serde_json::Value::Bool(*val),
            Node::AnonMap(mapping) => Self::map_to_json_obj(mapping),
            Node::Sub(mapping) => Self::map_to_json_obj(mapping),
            Node::Variant(variant_name, value) => {
                let mut out_mapping = serde_json::Map::new();
                out_mapping.insert(variant_name.to_owned(), value.to_json());
                serde_json::Value::Object(out_mapping)
            }
            Node::Enum(variant_name) => serde_json::Value::String(variant_name.to_owned()),
            Node::Optional(value) => match value {
                Some(value) => value.to_json(),
                None => serde_json::Value::Null,
            },
            Node::Const => serde_json::Value::Null,
        }
    }

    fn map_to_json_obj<T: ToString, N: Deref<Target = Node>>(
        map: &HashMap<T, N>,
    ) -> serde_json::Value {
        let mut out_mapping = serde_json::Map::new();
        for (key, val) in map {
            let out_key = key.to_string();
            let out_val = val.to_json();
            out_mapping.insert(out_key, out_val);
        }
        serde_json::Value::Object(out_mapping)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::HashMap;

    #[test]
    fn scenario() {
        let value = Value(Node::Sub(HashMap::from([
            ("a".to_string(), Box::new(Node::Real(2.0))),
            ("b".to_string(), Box::new(Node::Int(1))),
            (
                "c".to_string(),
                Box::new(Node::AnonMap(HashMap::from([(
                    0,
                    Box::new(Node::Bool(true)),
                )]))),
            ),
        ])));

        let expect_json_text = r#"{"a":2.0,"b":1,"c":{"0":true}}"#;
        assert_eq!(value.to_json().to_string(), expect_json_text);
    }
}
