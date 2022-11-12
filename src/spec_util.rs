use crate::common_util::format_path;
use crate::error::Error;
use crate::spec::{Node, Spec};
use crate::types::{HashMap, HashSet};

pub fn from_yaml_str(yaml_str: &str) -> Result<Spec, Error> {
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let root_path = [];
    Ok(Spec(build_node(&yaml_val, &root_path)?))
}

pub fn is_leaf(spec_node: &Node) -> bool {
    match spec_node {
        Node::Sub { .. } | Node::AnonMap { .. } | Node::Variant { .. } | Node::Optional { .. } => {
            false
        }
        Node::Bool { .. }
        | Node::Real { .. }
        | Node::Int { .. }
        | Node::Enum { .. }
        | Node::Const => true,
    }
}

fn build_node(yaml_val: &serde_yaml::Value, path: &[&str]) -> Result<Node, Error> {
    let mapping = match yaml_val {
        serde_yaml::Value::Mapping(mapping) => mapping,
        _ => {
            return Err(Error::ValueMustBeMap {
                path_hint: format_path(path),
            })
        }
    };

    let type_name = extract_string(mapping, "type", path, false)?;
    let type_name = type_name.as_deref().unwrap_or("sub");

    match type_name {
        "real" => build_real(mapping, path),
        "int" => build_int(mapping, path),
        "bool" => build_bool(mapping, path),
        "sub" => build_sub(mapping, path),
        "anon map" => build_anon_map(mapping, path),
        "variant" => build_variant(mapping, path),
        "enum" => build_enum(mapping, path),
        "optional" => build_optional(mapping, path),
        "const" => build_const(mapping, path),
        _ => Err(Error::UnknownTypeName {
            path_hint: format_path(path),
            unknown_type_name: type_name.to_string(),
        }),
    }
}

fn check_bounds_sanity<T: PartialOrd>(
    min: Option<T>,
    max: Option<T>,
    path: &[&str],
) -> Result<(), Error> {
    do_check_bounds_sanity(min, max, || Error::InvalidBounds {
        path_hint: format_path(path),
    })
}

fn check_size_bounds_sanity<T: PartialOrd>(
    min: Option<T>,
    max: Option<T>,
    path: &[&str],
) -> Result<(), Error> {
    do_check_bounds_sanity(min, max, || Error::InvalidSizeBounds {
        path_hint: format_path(path),
    })
}

fn do_check_bounds_sanity<F, T: PartialOrd>(
    min: Option<T>,
    max: Option<T>,
    error_supplier: F,
) -> Result<(), Error>
where
    F: FnOnce() -> Error,
{
    match (min, max) {
        (Some(min), Some(max)) if min >= max => Err(error_supplier()),
        _ => Ok(()),
    }
}

fn check_finite(num: f64, attribute_name: &str, path: &[&str]) -> Result<(), Error> {
    if !f64::is_finite(num) {
        Err(Error::NonFiniteNumber {
            path_hint: format_path(path),
            attribute_name: attribute_name.to_string(),
        })
    } else {
        Ok(())
    }
}

fn build_real(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "min", "max", "scale", "init"], path)?;

    let min = extract_real(mapping, "min", path, false)?;
    let max = extract_real(mapping, "max", path, false)?;

    check_bounds_sanity(min, max, path)?;

    let init = extract_real(mapping, "init", path, true)?.unwrap_or({
        let mut init = 0.0;
        min.iter().for_each(|min| {
            init = f64::max(*min, init);
        });
        max.iter().for_each(|max| {
            init = f64::min(*max, init);
        });
        init
    });

    if init < min.unwrap_or(init) || init > max.unwrap_or(init) {
        return Err(Error::InitNotWithinBounds {
            path_hint: format_path(path),
        });
    }

    let scale = extract_real(mapping, "scale", path, true)?.unwrap_or(1.0);
    check_scale(scale, path)?;

    Ok(Node::Real {
        init,
        scale,
        min,
        max,
    })
}

fn check_scale(scale: f64, path: &[&str]) -> Result<(), Error> {
    if scale <= 0. {
        Err(Error::ScaleMustBeStrictlyPositive {
            path_hint: format_path(path),
        })
    } else {
        Ok(())
    }
}

fn build_int(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "min", "max", "scale", "init"], path)?;

    let min = extract_int(mapping, "min", path, false)?;
    let max = extract_int(mapping, "max", path, false)?;

    check_bounds_sanity(min, max, path)?;

    let init = extract_int(mapping, "init", path, true)?.unwrap_or({
        let mut init = 0;
        min.iter().for_each(|min| {
            init = i64::max(*min, init);
        });
        max.iter().for_each(|max| {
            init = i64::min(*max, init);
        });
        init
    });

    if init < min.unwrap_or(init) || init > max.unwrap_or(init) {
        return Err(Error::InitNotWithinBounds {
            path_hint: format_path(path),
        });
    }

    let scale = extract_real(mapping, "scale", path, true)?.unwrap_or(1.0);
    check_scale(scale, path)?;

    Ok(Node::Int {
        init,
        scale,
        min,
        max,
    })
}

fn build_bool(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "init"], path)?;

    Ok(Node::Bool {
        init: extract_bool(mapping, "init", path, true)?.unwrap(),
    })
}

fn build_sub(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    let mut out_mapping = HashMap::default();
    for (key, value) in mapping {
        match key.as_str() {
            Some(attribute_key) if !attribute_key.eq("type") => {
                let path_of_sub = [path, &[attribute_key]].concat();
                out_mapping.insert(
                    attribute_key.to_string(),
                    Box::new(build_node(value, &path_of_sub)?),
                );
            }
            None => {
                return Err(Error::InvalidAttributeKeyType {
                    path_hint: format_path(path),
                    formatted_attribute_key: format_attribute_key(key),
                })
            }
            _ => (),
        }
    }

    if out_mapping.is_empty() {
        return Err(Error::EmptySub {
            path_hint: format_path(path),
        });
    }

    Ok(Node::Sub { map: out_mapping })
}

fn extract_value_type_attr_value(
    mapping: &serde_yaml::Mapping,
    path: &[&str],
) -> Result<Box<Node>, Error> {
    return match mapping.get("valueType") {
        Some(value) => Ok(Box::new(build_node(value, path)?)),
        None => {
            return Err(Error::MandatoryAttributeMissing {
                path_hint: format_path(path),
                missing_attribute_name: "valueType".to_string(),
            });
        }
    };
}

fn build_anon_map(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(
        mapping,
        ["type", "initSize", "minSize", "maxSize", "valueType"],
        path,
    )?;

    let value_type = extract_value_type_attr_value(mapping, path)?;

    let min_size = extract_usize_attribute_value(mapping, "minSize", path, false)?;
    let max_size = extract_usize_attribute_value(mapping, "maxSize", path, false)?;

    check_size_bounds_sanity(min_size, max_size, path)?;

    if max_size.filter(|max_size| *max_size == 0).is_some() {
        return Err(Error::ZeroMaxSize {
            path_hint: format_path(path),
        });
    }

    let init_size = extract_usize_attribute_value(mapping, "initSize", path, true)?.unwrap();

    let out_of_bounds = match (min_size, max_size) {
        (Some(min_size), _) if init_size < min_size => true,
        (_, Some(max_size)) if init_size > max_size => true,
        _ => false,
    };

    if out_of_bounds {
        Err(Error::InitSizeNotWithinBounds {
            path_hint: format_path(path),
        })
    } else {
        Ok(Node::AnonMap {
            value_type,
            init_size,
            min_size,
            max_size,
        })
    }
}

fn build_variant(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    let init_variant_name = extract_string(mapping, "init", path, true)?.unwrap();

    let mut out_mapping = HashMap::default();
    for (key, value) in mapping {
        match key.as_str() {
            Some(attribute_key) if !attribute_key.eq("type") && !attribute_key.eq("init") => {
                let path_of_sub = [path, &[attribute_key]].concat();
                out_mapping.insert(
                    attribute_key.to_string(),
                    Box::new(build_node(value, &path_of_sub)?),
                );
            }
            None => {
                return Err(Error::InvalidAttributeKeyType {
                    path_hint: format_path(path),
                    formatted_attribute_key: format_attribute_key(key),
                })
            }
            _ => (),
        }
    }

    if out_mapping.len() < 2 {
        return Err(Error::NotEnoughVariantValues {
            path_hint: format_path(path),
        });
    }

    if !out_mapping.contains_key(&init_variant_name) {
        return Err(Error::InitNotAKnownValue {
            path_hint: format_path(path),
            init: init_variant_name,
        });
    }

    Ok(Node::Variant {
        map: out_mapping,
        init: init_variant_name,
    })
}

fn build_enum(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "init", "values"], path)?;

    let mut values = Vec::new();
    let init_value = extract_string(mapping, "init", path, true)?.unwrap();

    let values_attr_name = "values".to_string();
    let values_sequence = match mapping.get(&values_attr_name) {
        None => {
            return Err(Error::MandatoryAttributeMissing {
                path_hint: format_path(path),
                missing_attribute_name: values_attr_name,
            })
        }
        Some(serde_yaml::Value::Sequence(values)) => values,
        _ => {
            return Err(Error::InvalidAttributeValueType {
                path_hint: format_path(path),
                attribute_name: values_attr_name,
                expected_type_hint: "a sequence".to_string(),
            })
        }
    };

    if values_sequence.len() < 2 {
        return Err(Error::NotEnoughEnumValues {
            path_hint: format_path(path),
        });
    }

    for name_value in values_sequence {
        if let serde_yaml::Value::String(name) = name_value {
            values.push(name.to_owned());
        } else {
            return Err(Error::EnumItemsMustBeString {
                path_hint: format_path(path),
            });
        }
    }

    if !values.contains(&init_value) {
        return Err(Error::InitNotAKnownValue {
            path_hint: format_path(path),
            init: init_value,
        });
    }

    Ok(Node::Enum {
        values,
        init: init_value,
    })
}

fn build_optional(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "initPresent", "valueType"], path)?;

    let value_type = extract_value_type_attr_value(mapping, path)?;
    let init_present = extract_bool(mapping, "initPresent", path, true)?.unwrap();

    Ok(Node::Optional {
        value_type,
        init_present,
    })
}

fn build_const(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type"], path)?;
    Ok(Node::Const)
}

fn extract_string(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    mandatory: bool,
) -> Result<Option<String>, Error> {
    extract_attribute_value(
        mapping,
        attribute_name,
        path,
        |value| value.as_str().map(|s| s.to_string()),
        "a string",
        mandatory,
    )
}

fn extract_real(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    mandatory: bool,
) -> Result<Option<f64>, Error> {
    let result = extract_attribute_value(
        mapping,
        attribute_name,
        path,
        |value| value.as_f64(),
        "a real number",
        mandatory,
    );

    match result? {
        res @ Some(num) => {
            check_finite(num, attribute_name, path)?;
            Ok(res)
        }
        res => Ok(res),
    }
}

fn extract_int(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    mandatory: bool,
) -> Result<Option<i64>, Error> {
    extract_attribute_value(
        mapping,
        attribute_name,
        path,
        |value| value.as_i64(),
        "an integer",
        mandatory,
    )
}

fn extract_bool(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    mandatory: bool,
) -> Result<Option<bool>, Error> {
    extract_attribute_value(
        mapping,
        attribute_name,
        path,
        |value| value.as_bool(),
        "a boolean",
        mandatory,
    )
}

fn extract_usize_attribute_value(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    mandatory: bool,
) -> Result<Option<usize>, Error> {
    let value_u64 = extract_attribute_value(
        mapping,
        attribute_name,
        path,
        |value| value.as_u64(),
        "a positive integer",
        mandatory,
    )?;

    match value_u64 {
        Some(val) => match usize::try_from(val) {
            Err(_) => Err(Error::UnsignedIntConversionFailed {
                path_hint: format_path(path),
                attribute_name: attribute_name.to_string(),
            }),
            Ok(res) => Ok(Some(res)),
        },
        _ => Ok(None),
    }
}

fn extract_attribute_value<F, T>(
    mapping: &serde_yaml::Mapping,
    attribute_name: &str,
    path: &[&str],
    value_extractor: F,
    expected_type_hint: &str,
    mandatory: bool,
) -> Result<Option<T>, Error>
where
    F: FnOnce(&serde_yaml::Value) -> Option<T>,
{
    let result = match mapping.get(attribute_name) {
        Some(value) => match value_extractor(value) {
            Some(value) => Some(value),
            None => {
                return Err(Error::InvalidAttributeValueType {
                    path_hint: format_path(path),
                    attribute_name: attribute_name.to_string(),
                    expected_type_hint: expected_type_hint.to_string(),
                });
            }
        },
        None => None,
    };

    match result {
        None if mandatory => Err(Error::MandatoryAttributeMissing {
            path_hint: format_path(path),
            missing_attribute_name: attribute_name.to_string(),
        }),
        _ => Ok(result),
    }
}

fn check_for_unexpected_attributes<const N: usize>(
    mapping: &serde_yaml::Mapping,
    allowed_attributes: [&str; N],
    path: &[&str],
) -> Result<(), Error> {
    let allowed_attributes = HashSet::from_iter(allowed_attributes);
    for attribute_key in mapping.keys() {
        match attribute_key {
            serde_yaml::Value::String(attribute_name) => {
                if !allowed_attributes.contains(attribute_name.as_str()) {
                    return Err(Error::UnexpectedAttribute {
                        path_hint: format_path(path),
                        unexpected_attribute_name: attribute_name.clone(),
                    });
                }
            }
            _ => {
                return Err(Error::InvalidAttributeKeyType {
                    path_hint: format_path(path),
                    formatted_attribute_key: format_attribute_key(attribute_key),
                });
            }
        }
    }

    Ok(())
}

fn format_attribute_key(key: &serde_yaml::Value) -> String {
    serde_yaml::to_string(key).unwrap().replace('\n', "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::approx_eq;
    use float_cmp::F64Margin;

    #[test]
    fn invalid_yaml() {
        let invalid_yaml_str = "{";
        assert!(matches!(
            from_yaml_str(invalid_yaml_str),
            Err(Error::InvalidYaml(_))
        ));
    }

    #[test]
    fn yaml_not_map() {
        let yaml_str = "foo";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::ValueMustBeMap { .. })
        ));
    }

    #[test]
    fn unknown_type_name() {
        let yaml_str = "
        type: foo
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::UnknownTypeName { path_hint, unknown_type_name })
            if path_hint == "(root)" && unknown_type_name == "foo"
        ));
    }

    #[test]
    fn const_node() {
        let yaml_str = "
        type: const
        ";

        assert!(matches!(from_yaml_str(yaml_str), Ok(Spec(Node::Const))))
    }

    #[test]
    fn const_unexpected_attribute() {
        let yaml_str = "
        type: const
        init: false
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::UnexpectedAttribute { path_hint, unexpected_attribute_name })
            if path_hint == "(root)" && unexpected_attribute_name == "init"
        ));
    }

    #[test]
    fn bool() {
        let yaml_str = "
        type: bool
        init: true
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Bool { init: true }))
        ));
    }

    #[test]
    fn real() {
        let yaml_str = "
        type: real
        init: 0.25
        scale: 0.1
        min: -1
        max: 1.6
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Real {
                min: Some(min),
                max: Some(max),
                init,
                scale,
            })) if
            approx_eq!(f64, min, -1.0, F64Margin::default()) &&
            approx_eq!(f64, max, 1.6, F64Margin::default()) &&
            approx_eq!(f64, init, 0.25, F64Margin::default()) &&
            approx_eq!(f64, scale, 0.1, F64Margin::default())
        ));
    }

    #[test]
    fn real_scale_not_strictly_positive() {
        let yaml_str = "
        type: real
        init: 0
        scale: 0.0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::ScaleMustBeStrictlyPositive { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn real_missing_scale() {
        let yaml_str = "
        type: real
        init: 0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::MandatoryAttributeMissing { path_hint, missing_attribute_name })
            if path_hint == "(root)" && missing_attribute_name == "scale"
        ));
    }

    #[test]
    fn real_missing_init() {
        let yaml_str = "
        type: real
        scale: 1
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::MandatoryAttributeMissing { path_hint, missing_attribute_name })
            if path_hint == "(root)" && missing_attribute_name == "init"
        ));
    }

    #[test]
    fn real_bounds_sanity() {
        let yaml_str = "
        type: real
        init: 0
        scale: 1
        min: 1
        max: 0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
        Err(Error::InvalidBounds {path_hint}) if *"(root)" == path_hint
        ));
    }

    #[test]
    fn non_finite_number() {
        let yaml_str = "
        type: real
        init: 0
        scale: .nan
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
        Err(Error::NonFiniteNumber {path_hint, attribute_name}) if *"(root)" == path_hint && *"scale" == attribute_name
        ));
    }

    #[test]
    fn real_defaults() {
        let yaml_str = "
        type: real
        init: 0.0
        scale: 1.0
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Real {
                min: None,
                max: None,
                init,
                scale,
            })) if
            approx_eq!(f64, init, 0.0, F64Margin::default()) &&
            approx_eq!(f64, scale, 1.0, F64Margin::default())
        ));
    }

    #[test]
    fn init_not_within_bounds() {
        let yaml_str = "
        type: real
        min: 0
        max: 1
        init: 2
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::InitNotWithinBounds { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn int() {
        let yaml_str = "
        type: int
        init: 2
        scale: 0.5
        min: -1
        max: 10
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Int {
                min: Some(-1),
                max: Some(10),
                init: 2,
                scale,
            })) if
            approx_eq!(f64, scale, 0.5, F64Margin::default())
        ));
    }

    #[test]
    fn int_missing_scale() {
        let yaml_str = "
        type: int
        init: 0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::MandatoryAttributeMissing { path_hint, missing_attribute_name })
            if path_hint == "(root)" && missing_attribute_name == "scale"
        ));
    }

    #[test]
    fn int_scale_not_positive() {
        let yaml_str = "
        type: int
        init: 0
        scale: -0.5
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::ScaleMustBeStrictlyPositive { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn int_missing_init() {
        let yaml_str = "
        type: int
        scale: 1
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::MandatoryAttributeMissing { path_hint, missing_attribute_name })
            if path_hint == "(root)" && missing_attribute_name == "init"
        ));
    }
    #[test]
    fn int_bounds_sanity() {
        let yaml_str = "
        type: int
        init: 0
        scale: 1
        min: 1
        max: 0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
        Err(Error::InvalidBounds {path_hint}) if *"(root)" == path_hint
        ));
    }

    #[test]
    fn int_defaults() {
        let yaml_str = "
        type: int
        init: 0
        scale: 1.0
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Int {
                min: None,
                max: None,
                init: 0,
                scale,
            })) if
            approx_eq!(f64, scale, 1.0, F64Margin::default())
        ));
    }

    #[test]
    fn sub() {
        let yaml_str = "
        foo:
            type: bool
            init: false
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Sub {
                map
            })) if map.len() == 1 &&
                *map.get("foo").unwrap().as_ref() == Node::Bool {init: false}
        ));
    }

    #[test]
    fn sub_empty() {
        let yaml_str = "
        type: sub
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::EmptySub {
                path_hint
            }) if path_hint == "(root)"
        ));
    }

    #[test]
    fn anon_map() {
        let yaml_str = "
        type: anon map
        valueType:
            type: bool
            init: false
        initSize: 2
        minSize: 2
        maxSize: 4
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::AnonMap {
                init_size: 2,
                value_type,
                min_size: Some(2),
                max_size: Some(4)
            })) if *value_type.as_ref() == Node::Bool {init: false}
        ));
    }

    #[test]
    fn anon_map_defaults() {
        let yaml_str = "
        type: anon map
        initSize: 1
        valueType:
            type: bool
            init: false
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::AnonMap {
                init_size: 1,
                value_type,
                min_size: None,
                max_size: None
            })) if *value_type.as_ref() == Node::Bool {init: false}
        ));
    }

    #[test]
    fn anon_map_zero_max_size() {
        let yaml_str = "
        type: anon map
        valueType:
            type: bool
            init: false
        maxSize: 0
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::ZeroMaxSize { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn anon_map_missing_value_type() {
        let yaml_str = "
        type: anon map
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::MandatoryAttributeMissing { path_hint, missing_attribute_name })
            if path_hint == "(root)" && missing_attribute_name == "valueType"
        ));
    }

    #[test]
    fn anon_map_invalid_size_bounds() {
        let yaml_str = "
        type: anon map
        valueType:
            type: bool
            init: false
        minSize: 3
        maxSize: 2
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::InvalidSizeBounds { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn anon_map_init_size_out_of_bounds() {
        let yaml_str = "
        type: anon map
        valueType:
            type: bool
            init: false
        minSize: 2
        maxSize: 3
        initSize: 4
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::InitSizeNotWithinBounds { path_hint })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn variant() {
        let yaml_str = "
        type: variant
        init: bar
        foo:
            type: bool
            init: false
        bar:
            type: int
            init: 0
            scale: 1
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Variant {
                map,
                init
            })) if map.len() == 2 &&
                *map.get("foo").unwrap().as_ref() == Node::Bool {init: false} &&
                matches!(*map.get("bar").unwrap().as_ref(), Node::Int {init: 0, ..}) &&
                init == *"bar"
        ));
    }

    #[test]
    fn variant_not_enough_values() {
        let yaml_str = "
        type: variant
        init: foo
        foo:
            type: bool
            init: false
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::NotEnoughVariantValues {
                path_hint
            }) if path_hint == "(root)"
        ));
    }

    #[test]
    fn variant_unknown_init() {
        let yaml_str = "
        type: variant
        init: bars
        foo:
            type: bool
            init: false
        bar:
            type: int
            init: 0
            scale: 1
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::InitNotAKnownValue {
                path_hint,
                init
            }) if path_hint == "(root)" && init == "bars"
        ));
    }

    #[test]
    fn enum_spec() {
        let yaml_str = "
        type: enum
        init: bar
        values:
        - foo
        - bar
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Enum {
                values,
                init
            })) if values == vec!["foo", "bar"] &&
                init == *"bar"
        ));
    }

    #[test]
    fn enum_not_enough_values() {
        let yaml_str = "
        type: enum
        init: foo
        values:
        - foo
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::NotEnoughEnumValues {
                path_hint
            }) if path_hint == "(root)"
        ));
    }

    #[test]
    fn enum_unknown_init() {
        let yaml_str = "
        type: enum
        init: bars
        values:
        - foo
        - bar
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::InitNotAKnownValue {
                path_hint,
                init
            }) if path_hint == "(root)" && init == "bars"
        ));
    }

    #[test]
    fn enum_items_must_be_string() {
        let yaml_str = "
        type: enum
        init: \"foo\"
        values:
        - 0
        - foo
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::EnumItemsMustBeString {
                path_hint
            }) if path_hint == "(root)"
        ));
    }

    #[test]
    fn optional() {
        let yaml_str = "
        type: optional
        valueType:
            type: bool
            init: true
        initPresent: true
        ";

        let expected = Node::Optional {
            value_type: Box::new(Node::Bool { init: true }),
            init_present: true,
        };

        assert_eq!(from_yaml_str(yaml_str).unwrap().0, expected);
    }

    #[test]
    fn optional_init_missing() {
        let yaml_str = "
        type: optional
        valueType:
            type: bool
            init: true
        ";

        if let Error::MandatoryAttributeMissing {
            path_hint,
            missing_attribute_name,
        } = from_yaml_str(yaml_str).unwrap_err()
        {
            assert_eq!(path_hint, "(root)");
            assert_eq!(missing_attribute_name, "initPresent");
        } else {
            panic!();
        }
    }

    #[test]
    fn unexpected_attribute() {
        let yaml_str = "
        type: anon map
        unexpected: false
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::UnexpectedAttribute { path_hint, unexpected_attribute_name })
            if path_hint == "(root)" && unexpected_attribute_name == "unexpected"
        ));
    }

    #[test]
    fn invalid_attribute_value_type() {
        let yaml_str = "
        type: anon map
        valueType:
            type: bool
            init: true
        maxSize: true
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::InvalidAttributeValueType { path_hint, attribute_name, expected_type_hint })
            if path_hint == "(root)" && attribute_name == "maxSize" && expected_type_hint == "a positive integer"
        ));
    }

    #[test]
    fn invalid_attribute_key_type() {
        let yaml_str = "
        1: 1
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::InvalidAttributeKeyType { path_hint, .. })
            if path_hint == "(root)"
        ));
    }

    #[test]
    fn path_hint() {
        let yaml_str = "
        foo:
            type: bar
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::UnknownTypeName { path_hint, .. })
            if path_hint == "foo"
        ));
    }
}
