use std::collections::HashMap;

use crate::value::Node::{self, *};
use crate::value::Value;

pub fn extract_as_real<'a>(value: &'a Value, path: &[&str]) -> Option<f64> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Real(value) = node {
            *value
        } else {
            panic!("Node is not of type real")
        }
    })
}

pub fn extract_as_int<'a>(value: &'a Value, path: &[&str]) -> Option<i64> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Int(value) = node {
            *value
        } else {
            panic!("Node is not of type int")
        }
    })
}

pub fn extract_as_bool<'a>(value: &'a Value, path: &[&str]) -> Option<bool> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::Bool(value) = node {
            *value
        } else {
            panic!("Node is not of type bool")
        }
    })
}

pub fn extract_as_anon_map<'a>(
    value: &'a Value,
    path: &[&str],
) -> Option<HashMap<usize, Box<Node>>> {
    extract_from_node(Some(&value.0), path).map(|node| {
        if let Node::AnonMap(mapping) = node {
            mapping.clone()
        } else {
            panic!("Node is not of type bool")
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
            AnonMap(mapping) => extract_from_node(
                mapping
                    .get(&str::parse(*head).expect("Invalid path"))
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
            Real { .. } | Int { .. } | Bool { .. } | Enum(_) => panic!("Invalid path"),
        },
        None => Some(node),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn example() {
        let value = Value(Sub(HashMap::from([(
            "foo".to_string(),
            Box::new(AnonMap(HashMap::from([(5, Box::new(Int(6)))]))),
        )])));

        assert_eq!(*extract_from_value(&value, &["foo", "5"]).unwrap(), Int(6));
    }
}
