use crate::common_util::format_path;
use crate::error::Error;
use crate::{
    spec,
    value::{Node, Value},
};
use std::collections::HashMap;

pub fn from_json_str(json_str: &str, spec: &spec::Spec) -> Result<Value, Error> {
    let json_val: serde_json::Value = serde_json::from_str(json_str)?;

    Ok(Value(build_node(Some(&json_val), &spec.0, &[])?.unwrap()))
}

fn build_node(
    json_val: Option<&serde_json::Value>,
    spec_node: &spec::Node,
    path: &[&str],
) -> Result<Option<Node>, Error> {
    match *spec_node {
        spec::Node::Real {
            ref optional,
            min,
            max,
            ..
        } => build_real(json_val, min, max, *optional, path),
        spec::Node::Int {
            ref optional,
            min,
            max,
            ..
        } => build_int(json_val, min, max, *optional, path),
        spec::Node::Bool { .. } => build_bool(json_val, path),
        spec::Node::Sub {
            ref optional,
            map: ref spec_map,
        } => build_sub(json_val, *optional, spec_map, path),
        spec::Node::AnonMap {
            ref optional,
            ref value_type,
            ..
        } => build_anon_map(json_val, *optional, value_type, path),
    }
}

fn build_real(
    json_val: Option<&serde_json::Value>,
    min: Option<f64>,
    max: Option<f64>,
    is_optional: bool,
    path: &[&str],
) -> Result<Option<Node>, Error> {
    match json_val {
        Some(serde_json::Value::Number(number)) => match number.as_f64() {
            None => Err(Error::NumberConversionFailed {
                path_hint: format_path(path),
            }),
            Some(real_num) => {
                let out_of_bounds = match (min, max) {
                    (Some(min), _) if real_num < min => true,
                    (_, Some(max)) if real_num > max => true,
                    _ => false,
                };

                if out_of_bounds {
                    Err(Error::ValueNotWithinBounds {
                        path_hint: format_path(path),
                    })
                } else {
                    Ok(Some(Node::Real(real_num)))
                }
            }
        },
        Some(_) => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "a real number".to_string(),
        }),
        None => {
            if is_optional {
                Ok(None)
            } else {
                Err(Error::MandatoryValueMissing {
                    path_hint: format_path(path),
                })
            }
        }
    }
}

fn build_int(
    json_val: Option<&serde_json::Value>,
    min: Option<i64>,
    max: Option<i64>,
    is_optional: bool,
    path: &[&str],
) -> Result<Option<Node>, Error> {
    match json_val {
        Some(serde_json::Value::Number(number)) => match number.as_i64() {
            None => Err(Error::NumberConversionFailed {
                path_hint: format_path(path),
            }),
            Some(real_num) => {
                let out_of_bounds = match (min, max) {
                    (Some(min), _) if real_num < min => true,
                    (_, Some(max)) if real_num > max => true,
                    _ => false,
                };

                if out_of_bounds {
                    Err(Error::ValueNotWithinBounds {
                        path_hint: format_path(path),
                    })
                } else {
                    Ok(Some(Node::Int(real_num)))
                }
            }
        },
        Some(_) => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "integer".to_string(),
        }),
        None => {
            if is_optional {
                Ok(None)
            } else {
                Err(Error::MandatoryValueMissing {
                    path_hint: format_path(path),
                })
            }
        }
    }
}

fn build_bool(json_val: Option<&serde_json::Value>, path: &[&str]) -> Result<Option<Node>, Error> {
    match json_val {
        Some(serde_json::Value::Bool(value)) => Ok(Some(Node::Bool(*value))),
        Some(_) => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "a boolean".to_string(),
        }),
        None => Err(Error::MandatoryValueMissing {
            path_hint: format_path(path),
        }),
    }
}

fn build_sub(
    json_val: Option<&serde_json::Value>,
    is_optional: bool,
    spec_map: &HashMap<String, Box<spec::Node>>,
    path: &[&str],
) -> Result<Option<Node>, Error> {
    match json_val {
        Some(serde_json::Value::Object(value_mapping)) => {
            let unexpected_key = value_mapping
                .iter()
                .map(|entry| entry.0)
                .find(|key| !spec_map.contains_key(*key));

            match unexpected_key {
                Some(key) => Err(Error::UnexpectedKey {
                    path_hint: format_path(path),
                    key: key.clone(),
                }),
                None => {
                    let mut result_mapping = HashMap::new();
                    for (spec_key, spec_node) in spec_map {
                        let json_val = value_mapping.get(spec_key);
                        let path_of_sub = [path, &[spec_key.as_str()]].concat();
                        let value_node = build_node(json_val, spec_node.as_ref(), &path_of_sub)?;

                        if let Some(value_node) = value_node {
                            result_mapping.insert(spec_key.clone(), Box::new(value_node));
                        }
                    }
                    Ok(Some(Node::Sub(result_mapping)))
                }
            }
        }
        Some(_) => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "map".to_string(),
        }),
        None => {
            if is_optional {
                Ok(None)
            } else {
                Err(Error::MandatoryValueMissing {
                    path_hint: format_path(path),
                })
            }
        }
    }
}

fn build_anon_map(
    json_val: Option<&serde_json::Value>,
    is_optional: bool,
    spec_node: &spec::Node,
    path: &[&str],
) -> Result<Option<Node>, Error> {
    match json_val {
        Some(serde_json::Value::Array(json_values)) => {
            let mut result_mapping = HashMap::new();

            for (next_id, json_val) in json_values.iter().enumerate() {
                let path_item = next_id.to_string();
                let path_of_sub = [path, &[&path_item]].concat();
                let value = build_node(Some(json_val), spec_node, &path_of_sub)?.unwrap();
                result_mapping.insert(next_id, Box::new(value));
            }

            Ok(Some(Node::AnonMap(result_mapping)))
        }
        Some(serde_json::Value::Object(json_mapping)) => {
            let mut result_mapping = HashMap::new();

            for (json_key, json_value) in json_mapping {
                let (key, path_item) = match str::parse::<usize>(json_key) {
                    Ok(key) => (key, json_key),
                    Err(_) => {
                        return Err(Error::InvalidAnonMapKey {
                            path_hint: format_path(path),
                        })
                    }
                };

                let path_of_sub = [path, &[path_item]].concat();
                let value = build_node(Some(json_value), spec_node, &path_of_sub)?.unwrap();
                result_mapping.insert(key, Box::new(value));
            }

            Ok(Some(Node::AnonMap(result_mapping)))
        }
        Some(_) => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "array or map".to_string(),
        }),
        None => {
            if is_optional {
                Ok(None)
            } else {
                Err(Error::MandatoryValueMissing {
                    path_hint: format_path(path),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        spec_util,
        value::{Node, Value},
    };
    use float_cmp::approx_eq;
    use float_cmp::F64Margin;

    #[test]
    fn invalid_json() {
        let spec_str = "
        type: bool
        ";
        let invalid_json_str = "{";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(invalid_json_str, &spec);

        assert!(matches!(result, Err(Error::InvalidJson(_))));
    }

    #[test]
    fn bool() {
        let spec_str = "
        type: bool
        ";
        let value_str = "true";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(matches!(result, Ok(Value(Node::Bool(true)))));
    }

    #[test]
    fn bool_wrong_value_type() {
        let spec_str = "
        type: bool
        ";
        let value_str = "4.0";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint == *"(root)" && type_hint == *"a boolean" )
        );
    }

    #[test]
    fn real() {
        let spec_str = "
        type: real
        init: 0
        scale: 1
        ";
        let value_str = "4.0";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Ok(Value(Node::Real(value))) if approx_eq!(f64, value, 4.0, F64Margin::default()))
        );
    }

    #[test]
    fn real_upper_bound_breach() {
        let spec_str = "
        type: real
        init: 3
        scale: 1
        min: 3
        max: 4
        ";
        let value_str = "4.01";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::ValueNotWithinBounds {path_hint}) if path_hint == *"(root)" )
        );
    }

    #[test]
    fn real_lower_bound_breach() {
        let spec_str = "
        type: real
        init: 3
        scale: 1
        min: 3
        max: 4
        ";
        let value_str = "2.99";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::ValueNotWithinBounds {path_hint}) if path_hint == *"(root)" )
        );
    }

    #[test]
    fn real_not_present() {
        let spec_str = "
        foo:
            type: real
            init: 0
            scale: 1
        bar:
            type: real
            init: 0
            scale: 1
        ";
        let value_str = "{
            \"bar\": 4.0
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::MandatoryValueMissing {path_hint}) if path_hint.as_str() == "foo")
        );
    }

    #[test]
    fn real_wrong_value_type() {
        let spec_str = "
        type: real
        init: 0
        scale: 1
        ";
        let value_str = "{ \"foo\": 4.0}";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint == *"(root)" && type_hint == *"a real number" )
        );
    }

    #[test]
    fn int() {
        let spec_str = "
        type: int
        init: 0
        scale: 1
        min: 0
        max: 5
        ";
        let value_str = "5";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(matches!(result, Ok(Value(Node::Int(5)))));
    }

    #[test]
    fn int_upper_bound_breach() {
        let spec_str = "
        type: int
        init: 3
        scale: 1
        min: 3
        max: 4
        ";
        let value_str = "5";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::ValueNotWithinBounds {path_hint}) if path_hint == *"(root)" )
        );
    }

    #[test]
    fn int_lower_bound_breach() {
        let spec_str = "
        type: int
        init: 3
        scale: 1
        min: 3
        max: 4
        ";
        let value_str = "2";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::ValueNotWithinBounds {path_hint}) if path_hint == *"(root)" )
        );
    }

    #[test]
    fn int_not_present() {
        let spec_str = "
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: int
            init: 0
            scale: 1
        ";
        let value_str = "{
            \"bar\": 4
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::MandatoryValueMissing {path_hint}) if path_hint.as_str() == "foo")
        );
    }

    #[test]
    fn int_wrong_value_type() {
        let spec_str = "
        type: int
        init: 0
        scale: 1
        ";
        let value_str = "false";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint == *"(root)" && type_hint == *"integer" )
        );
    }

    #[test]
    fn int_number_conversion_failed() {
        let spec_str = "
        type: int
        init: 0
        scale: 1
        ";
        let value_str = "2.5";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::NumberConversionFailed { path_hint }) if path_hint == *"(root)" )
        );
    }

    #[test]
    fn sub() {
        let spec_str = "
        foo:
            type: int
            init: 0
            scale: 1
        ";
        let value_str = "{
            \"foo\": 4
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert_eq!(
            result.unwrap(),
            Value(Node::Sub(HashMap::from([(
                "foo".to_owned(),
                Box::new(Node::Int(4))
            )])))
        );
    }

    #[test]
    fn sub_unexpected_key() {
        let spec_str = "
        foo:
            type: int
            init: 0
            scale: 1
        ";
        let value_str = "{
            \"bar\": 4
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);
        assert!(
            matches!(result, Err(Error::UnexpectedKey { path_hint, key }) if path_hint.as_str() == "(root)" && key.as_str() == "bar"
            )
        );
    }

    #[test]
    fn sub_wrong_type_for_value() {
        let spec_str = "
        foo:
            type: int
            init: 0
            scale: 1
        ";
        let value_str = "false";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);
        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint.as_str() == "(root)" && type_hint.as_str() == "map"
            )
        );
    }

    #[test]
    fn sub_not_present() {
        let spec_str = "
        foo:
            bar:
                type: int
                init: 0
                scale: 1
        bar:
            type: int
            init: 0
            scale: 1
        ";
        let value_str = "{
            \"bar\": 4
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::MandatoryValueMissing {path_hint}) if path_hint.as_str() == "foo")
        );
    }

    #[test]
    fn anon_map_no_keys() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
        ";
        let value_str = "
        [true]
        ";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert_eq!(
            result.unwrap(),
            Value(Node::AnonMap(HashMap::from([(
                0usize,
                Box::new(Node::Bool(true))
            )])))
        );
    }

    #[test]
    fn anon_map_with_keys() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
        ";
        let value_str = "{
        \"2\": true
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert_eq!(
            result.unwrap(),
            Value(Node::AnonMap(HashMap::from([(
                2usize,
                Box::new(Node::Bool(true))
            )])))
        );
    }

    #[test]
    fn anon_map_invalid_key() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
        ";
        let value_str = "{
        \"foo\": true
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::InvalidAnonMapKey { path_hint }) if path_hint.as_str() == "(root)")
        );
    }

    #[test]
    fn anon_map_wrong_type() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
        ";
        let value_str = "
        3
        ";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint.as_str() == "(root)" && type_hint.as_str() == "array or map"
            )
        );
    }

    #[test]
    fn complex_example_success() {
        let spec_str = "
        l0a:
            type: anon map
            valueType:
                type: sub
                foo:
                    type: int
                    init: 0
                    scale: 1
                bar:
                    type: real
                    optional: true
                    init: 0
                    scale: 1
        l0b:
            type: bool
        ";
        let value_str = "{
            \"l0a\": [
                {
                    \"foo\": 2,
                    \"bar\": 0.2
                },
                {
                    \"foo\": 5
                }
            ],
            \"l0b\": true
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(matches!(result, Ok(_)));
    }

    #[test]
    fn complex_example_wrong_type() {
        let spec_str = "
        l0a:
            type: anon map
            valueType:
                type: sub
                foo:
                    type: int
                    init: 0
                    scale: 1
                bar:
                    type: real
                    optional: true
                    init: 0
                    scale: 1
        l0b:
            type: bool
        ";
        let value_str = "{
            \"l0a\": [
                {
                    \"foo\": 2,
                    \"bar\": 0.2
                },
                {
                    \"foo\": false
                }
            ],
            \"l0b\": true
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(matches!(
            result,
            Err(Error::WrongTypeForValue {
                path_hint,
                type_hint
            }) if path_hint.as_str() == "l0a.1.foo" && type_hint.as_str() == "integer"
        ));
    }
}
