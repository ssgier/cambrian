use crate::common_util::format_path;
use crate::error::Error;
use crate::{
    spec,
    value::{Node, Value},
};
use std::collections::HashMap;

pub fn from_json_str(json_str: &str, spec: &spec::Spec) -> Result<Value, Error> {
    let json_val: serde_json::Value = serde_json::from_str(json_str)?;

    Ok(Value(build_node(&json_val, &spec.0, &[])?))
}

fn build_node(
    json_val: &serde_json::Value,
    spec_node: &spec::Node,
    path: &[&str],
) -> Result<Node, Error> {
    match *spec_node {
        spec::Node::Real { min, max, .. } => build_real(json_val, min, max, path),
        spec::Node::Int { min, max, .. } => build_int(json_val, min, max, path),
        spec::Node::Bool { .. } => build_bool(json_val, path),
        spec::Node::Sub { map: ref spec_map } => build_sub(json_val, spec_map, path),
        spec::Node::AnonMap { ref value_type, .. } => build_anon_map(json_val, value_type, path),
        spec::Node::Variant {
            map: ref spec_map, ..
        } => build_variant(json_val, spec_map, path),
        spec::Node::Enum { ref values, .. } => build_enum(json_val, values, path),
        spec::Node::Optional { ref value_type, .. } => build_optional(json_val, value_type, path),
    }
}

fn build_real(
    json_val: &serde_json::Value,
    min: Option<f64>,
    max: Option<f64>,
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Number(number) => match number.as_f64() {
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
                    Ok(Node::Real(real_num))
                }
            }
        },
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "a real number".to_string(),
        }),
    }
}

fn build_int(
    json_val: &serde_json::Value,
    min: Option<i64>,
    max: Option<i64>,
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Number(number) => match number.as_i64() {
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
                    Ok(Node::Int(real_num))
                }
            }
        },
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "integer".to_string(),
        }),
    }
}

fn build_bool(json_val: &serde_json::Value, path: &[&str]) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Bool(value) => Ok(Node::Bool(*value)),
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "a boolean".to_string(),
        }),
    }
}

fn build_sub(
    json_val: &serde_json::Value,
    spec_map: &HashMap<String, Box<spec::Node>>,
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Object(value_mapping) => {
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
                        let path_of_sub = [path, &[spec_key.as_str()]].concat();
                        let json_val =
                            value_mapping
                                .get(spec_key)
                                .map(Result::Ok)
                                .unwrap_or_else(|| {
                                    Err(Error::MandatoryValueMissing {
                                        path_hint: format_path(&path_of_sub),
                                    })
                                })?;

                        let value_node = build_node(json_val, spec_node.as_ref(), &path_of_sub)?;

                        result_mapping.insert(spec_key.clone(), Box::new(value_node));
                    }
                    Ok(Node::Sub(result_mapping))
                }
            }
        }
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "map".to_string(),
        }),
    }
}

fn build_anon_map(
    json_val: &serde_json::Value,
    spec_node: &spec::Node,
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Array(json_values) => {
            let mut result_mapping = HashMap::new();

            for (next_id, json_val) in json_values.iter().enumerate() {
                let path_item = next_id.to_string();
                let path_of_sub = [path, &[&path_item]].concat();
                let value = build_node(json_val, spec_node, &path_of_sub)?;
                result_mapping.insert(next_id, Box::new(value));
            }

            Ok(Node::AnonMap(result_mapping))
        }
        serde_json::Value::Object(json_mapping) => {
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
                let value = build_node(json_value, spec_node, &path_of_sub)?;
                result_mapping.insert(key, Box::new(value));
            }

            Ok(Node::AnonMap(result_mapping))
        }
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "array or map".to_string(),
        }),
    }
}

fn build_variant(
    json_val: &serde_json::Value,
    spec_map: &HashMap<String, Box<spec::Node>>,
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::Object(value_mapping) if value_mapping.len() == 1 => {
            let (variant_name, variant_value) = value_mapping.iter().next().unwrap();

            if let Some(child_spec_node) = spec_map.get(variant_name) {
                let path_of_sub = [path, &[variant_name.as_str()]].concat();
                let child_value_node = build_node(variant_value, child_spec_node, &path_of_sub)?;
                Ok(Node::Variant(
                    variant_name.to_owned(),
                    Box::new(child_value_node),
                ))
            } else {
                Err(Error::UnknownVariant {
                    path_hint: format_path(path),
                    variant_name: variant_name.to_owned(),
                })
            }
        }
        serde_json::Value::Object(_) => Err(Error::OnlyOneVariantAllowed {
            path_hint: format_path(path),
        }),
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "map".to_string(),
        }),
    }
}

fn build_enum(
    json_val: &serde_json::Value,
    variant_values: &[String],
    path: &[&str],
) -> Result<Node, Error> {
    match json_val {
        serde_json::Value::String(variant_value) => {
            if variant_values.contains(variant_value) {
                Ok(Node::Enum(variant_value.to_owned()))
            } else {
                Err(Error::UnknownValue {
                    path_hint: format_path(path),
                    value: variant_value.to_owned(),
                })
            }
        }
        _ => Err(Error::WrongTypeForValue {
            path_hint: format_path(path),
            type_hint: "string".to_string(),
        }),
    }
}

fn build_optional(
    json_val: &serde_json::Value,
    value_type: &spec::Node,
    path: &[&str],
) -> Result<Node, Error> {
    Ok(match json_val {
        serde_json::Value::Null => Node::Optional(None),
        present_value => {
            Node::Optional(Some(Box::new(build_node(present_value, value_type, path)?)))
        }
    })
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
        init: false
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
        init: false
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
        init: false
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
    fn variant() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: true
        ";
        let value_str = "{
            \"bar\": false
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        let expected = Node::Variant("bar".to_string(), Box::new(Node::Bool(false)));

        assert_eq!(result.unwrap(), Value(expected));
    }

    #[test]
    fn variant_unknown() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: true
        ";
        let value_str = "{
            \"bars\": false
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::UnknownVariant { path_hint, variant_name }) if path_hint.as_str() == "(root)" && variant_name == "bars")
        );
    }

    #[test]
    fn variant_only_one_allowed() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: true
        ";
        let value_str = "{
            \"foo\": 1,
            \"bar\": false
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::OnlyOneVariantAllowed { path_hint }) if path_hint.as_str() == "(root)")
        );
    }

    #[test]
    fn variant_wrong_type() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: true
        ";
        let value_str = "{
            \"foo\": false
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::WrongTypeForValue { path_hint, type_hint }) if path_hint.as_str() == "foo" && type_hint == "integer")
        );
    }

    #[test]
    fn enum_value() {
        let spec_str = "
        type: enum
        init: foo
        values:
        - foo
        - bar
        ";
        let value_str = "\"bar\"";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        let expected = Node::Enum("bar".to_string());

        assert_eq!(result.unwrap(), Value(expected));
    }

    #[test]
    fn enum_unknown_value() {
        let spec_str = "
        type: enum
        init: foo
        values:
        - foo
        - bar
        ";
        let value_str = "\"bars\"";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result, Err(Error::UnknownValue { path_hint, value }) if path_hint.as_str() == "(root)" && value == "bars")
        );
    }

    #[test]
    fn optional_absent() {
        let spec_str = "
        type: optional
        valueType:
            type: bool
            init: false
        initPresent: true
        ";
        let value_str = "null";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert_eq!(result.unwrap().0, Node::Optional(None));
    }

    #[test]
    fn optional_present() {
        let spec_str = "
        type: optional
        valueType:
            type: bool
            init: false
        initPresent: true
        ";
        let value_str = "true";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert_eq!(
            result.unwrap().0,
            Node::Optional(Some(Box::new(Node::Bool(true))))
        );
    }

    #[test]
    fn optional_wrong_type() {
        let spec_str = "
        type: optional
        valueType:
            type: bool
            init: false
        initPresent: true
        ";
        let value_str = "1";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result,
                     Err(Error::WrongTypeForValue { path_hint, type_hint })
                     if path_hint.as_str() == "(root)" && type_hint == "a boolean"));
    }

    #[test]
    fn optional_missing_null() {
        let spec_str = "
        foo:
            type: optional
            initPresent: false
            valueType:
                type: bool
                init: false
        ";
        let value_str = "{}";
        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let result = from_json_str(value_str, &spec);

        assert!(
            matches!(result,
                     Err(Error::MandatoryValueMissing { path_hint })
                     if path_hint.as_str() == "foo"));
    }

    #[test]
    fn anon_map_no_keys() {
        let spec_str = "
        type: anon map
        initSize: 1
        valueType:
            type: bool
            init: false
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
        initSize: 1
        valueType:
            type: bool
            init: false
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
        initSize: 1
        valueType:
            type: bool
            init: false
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
        initSize: 1
        valueType:
            type: bool
            init: false
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
            initSize: 1
            valueType:
                type: sub
                foo:
                    type: int
                    init: 0
                    scale: 1
                bar:
                    type: optional
                    initPresent: true
                    valueType:
                        type: real
                        init: 0
                        scale: 1
                foo_variant:
                    type: optional
                    initPresent: false
                    valueType:
                        type: variant
                        init: foo
                        foo:
                            type: enum
                            init: bar
                            values:
                            - foo
                            - bar
                        bar:
                            type: bool
                            init: false
        l0b:
            type: bool
            init: false
        ";
        let value_str = "{
            \"l0a\": [
                {
                    \"foo\": 2,
                    \"bar\": 0.2,
                    \"foo_variant\": null
                },
                {
                    \"foo\": 5,
                    \"bar\": null,
                    \"foo_variant\": {
                        \"foo\": \"foo\"
                    }
                },
                {
                    \"foo\": 5,
                    \"bar\": null,
                    \"foo_variant\": {
                        \"bar\": true
                    }
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
            initSize: 1
            valueType:
                type: sub
                foo:
                    type: int
                    init: 0
                    scale: 1
                bar:
                    type: optional
                    initPresent: true
                    valueType:
                        type: real
                        init: 0
                        scale: 1
        l0b:
            type: bool
            init: false
        ";
        let value_str = "{
            \"l0a\": [
                {
                    \"foo\": 2,
                    \"bar\": 0.2
                },
                {
                    \"foo\": false,
                    \"bar\": null
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
