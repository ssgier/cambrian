use crate::types::HashMap;
use crate::value::Node::{self, *};
use crate::value::Value;

pub fn extract_as_real(value: &Value, path: &[&str]) -> Option<f64> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Real(value) = node {
            *value
        } else {
            panic!("Node is not of type real")
        }
    })
}

pub fn extract_as_int(value: &Value, path: &[&str]) -> Option<i64> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Int(value) = node {
            *value
        } else {
            panic!("Node is not of type int")
        }
    })
}

pub fn extract_as_bool(value: &Value, path: &[&str]) -> Option<bool> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Bool(value) = node {
            *value
        } else {
            panic!("Node is not of type bool")
        }
    })
}

pub fn extract_as_array(value: &Value, path: &[&str]) -> Option<Vec<Node>> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Array(elements) = node {
            elements.iter().map(|elem| *elem.clone()).collect()
        } else {
            panic!("Node is not of type array")
        }
    })
}

pub fn extract_as_anon_map(value: &Value, path: &[&str]) -> Option<HashMap<usize, Box<Node>>> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::AnonMap(mapping) = node {
            mapping.clone()
        } else {
            panic!("Node is not of type anon map")
        }
    })
}

pub fn extract_from_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Node> {
    extract_from_node(Some(&value.0), path)
}

pub fn extract_from_node<'a>(node: Option<&'a Node>, path: &[&str]) -> Option<&'a Node> {
    node.and_then(|node| match path.first() {
        Some(head) => match node {
            Sub(mapping) => extract_from_node(mapping.get(*head).map(Box::as_ref), &path[1..]),
            Array(elements) => extract_from_node(
                elements
                    .get(str::parse::<usize>(head).expect("Invalid path"))
                    .map(Box::as_ref),
                &path[1..],
            ),
            AnonMap(mapping) => extract_from_node(
                mapping
                    .get(&str::parse(head).expect("Invalid path"))
                    .map(Box::as_ref),
                &path[1..],
            ),
            Variant(variant_name, value) => {
                if head != variant_name {
                    panic!("Invalid path");
                }
                extract_from_node(Some(value), &path[1..])
            }
            Optional(value_option) => {
                if *head != "optional" {
                    panic!("Invalid path");
                }
                extract_from_node(
                    value_option.as_ref().map(|boxed| boxed.as_ref()),
                    &path[1..],
                )
            }
            Real { .. } | Int { .. } | Bool { .. } | Enum(_) | Const => panic!("Invalid path"),
        },
        None => Some(node),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HashMap;

    #[test]
    fn example() {
        let value = Value(Sub(HashMap::from_iter([(
            "foo".to_string(),
            Box::new(AnonMap(HashMap::from_iter([(5, Box::new(Int(6)))]))),
        )])));

        assert_eq!(*extract_from_value(&value, &["foo", "5"]).unwrap(), Int(6));
    }
}
