use crate::types::HashMap;
use crate::value;
use crate::value::Value;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spec(pub Node);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Node {
    Real {
        init: f64,
        scale: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Int {
        init: i64,
        scale: f64,
        min: Option<i64>,
        max: Option<i64>,
    },
    Bool {
        init: bool,
    },
    Sub {
        map: HashMap<String, Box<Node>>,
    },
    AnonMap {
        value_type: Box<Node>,
        init_size: usize,
        min_size: Option<usize>,
        max_size: Option<usize>,
    },
    Variant {
        map: HashMap<String, Box<Node>>,
        init: String,
    },
    Enum {
        values: Vec<String>,
        init: String,
    },
    Optional {
        value_type: Box<Node>,
        init_present: bool,
    },
    Const,
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
                    HashMap::default()
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
            Node::Variant { map, init } => value::Node::Variant(
                init.to_owned(),
                Box::new(map.get(init).unwrap().initial_value()),
            ),
            Node::Enum { init, .. } => value::Node::Enum(init.to_owned()),
            Node::Optional {
                value_type,
                init_present,
            } => {
                let result_value = if *init_present {
                    Some(Box::new(value_type.initial_value()))
                } else {
                    None
                };

                value::Node::Optional(result_value)
            }
            Node::Const => value::Node::Const,
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
        d:
            type: variant
            init: foo
            foo:
                type: const
            bar:
                type: bool
                init: false
        ";

        let expected_init_val = Value(Node::Sub(HashMap::from_iter([
            ("a".to_string(), Box::new(Node::Real(2.0))),
            ("b".to_string(), Box::new(Node::Int(1))),
            (
                "c".to_string(),
                Box::new(Node::AnonMap(HashMap::from_iter([(
                    0,
                    Box::new(Node::Bool(true)),
                )]))),
            ),
            (
                "d".to_string(),
                Box::new(Node::Variant("foo".to_string(), Box::new(Node::Const))),
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

        let expected_init_val = Value(Node::AnonMap(HashMap::default()));
        let init_val = from_yaml_str(spec_str).unwrap().initial_value();

        assert_eq!(init_val, expected_init_val);
    }
}
