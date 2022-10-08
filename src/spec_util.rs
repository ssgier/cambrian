use crate::common_util::format_path;
use crate::error::Error;
use crate::spec::{IntProbDist, Node, RealProbDist, Spec};
use std::collections::{HashMap, HashSet};

pub fn from_yaml_str(yaml_str: &str) -> Result<Spec, Error> {
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;
    let root_path = [];
    Ok(Spec(build_node(&yaml_val, &root_path)?))
}

pub fn is_leaf(spec_node: &Node) -> bool {
    match spec_node {
        Node::Sub { .. } | Node::AnonMap { .. } => false,
        Node::Bool { .. } | Node::Real { .. } | Node::Int { .. } => true,
    }
}

pub fn is_optional(spec_node: &Node) -> bool {
    match *spec_node {
        Node::Bool { .. } => false,
        Node::Real { optional, .. } => optional,
        Node::Int { optional, .. } => optional,
        Node::Sub { optional, .. } => optional,
        Node::AnonMap { optional, .. } => optional,
    }
}

fn build_node(yaml_val: &serde_yaml::Value, path: &[&str]) -> Result<Node, Error> {
    let mapping = match yaml_val {
        serde_yaml::Value::Mapping(mapping) => mapping,
        _ => {
            return Err(Error::YamlMustBeMap {
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
    check_for_unexpected_attributes(
        mapping,
        ["type", "optional", "min", "max", "scale", "init", "dist"],
        path,
    )?;

    let optional = extract_is_optional(mapping, path)?;
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

    let prob_dist = extract_string(mapping, "dist", path, false)?;
    let prob_dist = match prob_dist.as_deref().unwrap_or("normal") {
        "normal" => RealProbDist::Normal,
        "exponential" => RealProbDist::Exponential,
        unknown_value => {
            return Err(Error::UnknownProbDist {
                path_hint: format_path(path),
                unknown_value: unknown_value.to_string(),
            })
        }
    };

    Ok(Node::Real {
        optional,
        init,
        scale,
        min,
        max,
        prob_dist,
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
    check_for_unexpected_attributes(
        mapping,
        ["type", "optional", "min", "max", "scale", "init", "dist"],
        path,
    )?;

    let optional = extract_is_optional(mapping, path)?;
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

    let prob_dist = extract_string(mapping, "dist", path, false)?;
    let prob_dist = match prob_dist.as_deref().unwrap_or("normal") {
        "normal" => IntProbDist::Normal,
        "uniform" => IntProbDist::Uniform,
        unknown_value => {
            return Err(Error::UnknownProbDist {
                path_hint: format_path(path),
                unknown_value: unknown_value.to_string(),
            })
        }
    };

    Ok(Node::Int {
        optional,
        init,
        scale,
        min,
        max,
        prob_dist,
    })
}

fn build_bool(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(mapping, ["type", "init"], path)?;

    Ok(Node::Bool {
        init: extract_bool(mapping, "init", path, false)?.unwrap_or(false),
    })
}

fn build_sub(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    let optional = extract_is_optional(mapping, path)?;

    let mut out_mapping = HashMap::new();
    for (key, value) in mapping {
        match key.as_str() {
            Some(attribute_key) if !attribute_key.eq("optional") && !attribute_key.eq("type") => {
                let path_of_sub = [path, &[attribute_key]].concat();
                out_mapping.insert(
                    attribute_key.to_string(),
                    Box::new(build_node(value, &path_of_sub)?),
                );
            }
            None => {
                return Err(Error::InvalidAttributeKeyType {
                    path_hint: format_path(path),
                    formatted_attribute_key: format!("{:?}", key),
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

    Ok(Node::Sub {
        optional,
        map: out_mapping,
    })
}

fn build_anon_map(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<Node, Error> {
    check_for_unexpected_attributes(
        mapping,
        [
            "type",
            "optional",
            "initSize",
            "minSize",
            "maxSize",
            "valueType",
        ],
        path,
    )?;

    let optional = extract_is_optional(mapping, path)?;

    let value_type = match mapping.get("valueType") {
        Some(value) => {
            let path_of_sub = [path, &["(anonymous)"]].concat();
            Box::new(build_node(value, &path_of_sub)?)
        }
        None => {
            return Err(Error::MandatoryAttributeMissing {
                path_hint: format_path(path),
                missing_attribute_name: "valueType".to_string(),
            });
        }
    };

    let min_size = extract_usize_attribute_value(mapping, "minSize", path, false)?;
    let max_size = extract_usize_attribute_value(mapping, "maxSize", path, false)?;

    check_size_bounds_sanity(min_size, max_size, path)?;

    let init_size = extract_usize_attribute_value(mapping, "initSize", path, false)?
        .or(min_size)
        .unwrap_or(0);

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
            optional,
            value_type,
            init_size,
            min_size,
            max_size,
        })
    }
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

fn extract_is_optional(mapping: &serde_yaml::Mapping, path: &[&str]) -> Result<bool, Error> {
    Ok(extract_bool(mapping, "optional", path, false)?.unwrap_or(false))
}

fn check_for_unexpected_attributes<const N: usize>(
    mapping: &serde_yaml::Mapping,
    allowed_attributes: [&str; N],
    path: &[&str],
) -> Result<(), Error> {
    let allowed_attributes = HashSet::from(allowed_attributes);
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
                    formatted_attribute_key: format!("{:?}", attribute_key),
                });
            }
        }
    }

    Ok(())
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
            Err(Error::YamlMustBeMap { .. })
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
    fn bool_default() {
        let yaml_str = "
        type: bool
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Bool { init: false }))
        ));
    }

    #[test]
    fn real() {
        let yaml_str = "
        type: real
        optional: true
        init: 0.25
        scale: 0.1
        min: -1
        max: 1.6
        dist: exponential
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Real {
                optional: true,
                min: Some(min),
                max: Some(max),
                prob_dist: RealProbDist::Exponential,
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
                optional: false,
                min: None,
                max: None,
                prob_dist: RealProbDist::Normal,
                init,
                scale,
            })) if
            approx_eq!(f64, init, 0.0, F64Margin::default()) &&
            approx_eq!(f64, scale, 1.0, F64Margin::default())
        ));
    }

    #[test]
    fn unknown_prob_dist() {
        let yaml_str = "
        type: real
        dist: Foo
        init: 0.1
        scale: 1.0
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Err(Error::UnknownProbDist { path_hint, unknown_value })
            if path_hint == "(root)" && unknown_value == "Foo"
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
        optional: true
        init: 2
        scale: 0.5
        min: -1
        max: 10
        dist: uniform
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Int {
                optional: true,
                min: Some(-1),
                max: Some(10),
                prob_dist: IntProbDist::Uniform,
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
                optional: false,
                min: None,
                max: None,
                prob_dist: IntProbDist::Normal,
                init: 0,
                scale,
            })) if
            approx_eq!(f64, scale, 1.0, F64Margin::default())
        ));
    }

    #[test]
    fn sub() {
        let yaml_str = "
        optional: true
        foo:
            type: bool
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Sub {
                optional: true,
                map
            })) if map.len() == 1 &&
                *map.get("foo").unwrap().as_ref() == Node::Bool {init: false}
        ));
    }

    #[test]
    fn sub_default() {
        let yaml_str = "
        subItem:
            type: bool
        ";
        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::Sub {
                optional: false,
                ..
            }))
        ));
    }

    #[test]
    fn sub_empty() {
        let yaml_str = "
        optional: true
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
        optional: true
        valueType:
            type: bool
        initSize: 2
        minSize: 2
        maxSize: 4
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::AnonMap {
                optional: true,
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
        valueType:
            type: bool
        ";

        assert!(matches!(
            from_yaml_str(yaml_str),
            Ok(Spec(Node::AnonMap {
                optional: false,
                init_size: 0,
                value_type,
                min_size: None,
                max_size: None
            })) if *value_type.as_ref() == Node::Bool {init: false}
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
        optional: 1
        ";

        assert!(matches!(
        from_yaml_str(yaml_str),
            Err(Error::InvalidAttributeValueType { path_hint, attribute_name, expected_type_hint })
            if path_hint == "(root)" && attribute_name == "optional" && expected_type_hint == "a boolean"
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
