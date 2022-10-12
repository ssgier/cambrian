use crate::value;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spec(pub Node);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Node {
    Real {
        optional: bool,
        init: f64,
        scale: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Int {
        optional: bool,
        init: i64,
        scale: f64,
        min: Option<i64>,
        max: Option<i64>,
    },
    Bool {
        init: bool,
    },
    Sub {
        optional: bool,
        map: HashMap<String, Box<Node>>,
    },
    AnonMap {
        optional: bool,
        value_type: Box<Node>,
        init_size: usize,
        min_size: Option<usize>,
        max_size: Option<usize>,
    },
}

impl Spec {
    pub fn initial_value(&self) -> Value {
        Value(self.0.initial_value())
    }
}

impl Node {
    pub fn initial_value(&self) -> value::Node {
        match self {
            Node::Real { init, .. } => value::Node::Real(*init),
            Node::Int { init, .. } => value::Node::Int(*init),
            Node::Bool { init } => value::Node::Bool(*init),
            Node::AnonMap {
                value_type,
                init_size,
                ..
            } => {
                let mapping = if *init_size == 0 {
                    HashMap::new()
                } else {
                    let init_val = value_type.initial_value();

                    (0..*init_size)
                        .map(|key| (key, Box::new(init_val.clone())))
                        .collect()
                };

                value::Node::AnonMap(mapping)
            }
            Node::Sub { map, .. } => {
                let out_map = map
                    .iter()
                    .map(|entry| {
                        let key = entry.0.clone();
                        let val = entry.1.initial_value();
                        (key, Box::new(val))
                    })
                    .collect();

                value::Node::Sub(out_map)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::spec_util::from_yaml_str;
    use value::Node;

    use super::*;

    #[test]
    fn scenario() {
        let spec_str = "
        a:
            type: real
            init: 2.0
            scale: 1.0
        b:
            type: int
            init: 1
            scale: 1
        c:
            type: anon map
            valueType:
                type: bool
                init: true
            initSize: 1 
        ";

        let expected_init_val = Value(Node::Sub(HashMap::from([
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

        let init_val = from_yaml_str(spec_str).unwrap().initial_value();

        assert_eq!(init_val, expected_init_val);
    }

    #[test]
    fn anon_map_zero_init_size() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
            init: true
        initSize: 0
        ";

        let expected_init_val = Value(Node::AnonMap(HashMap::new()));
        let init_val = from_yaml_str(spec_str).unwrap().initial_value();

        assert_eq!(init_val, expected_init_val);
    }
}
