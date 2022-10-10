use crate::value::Node::{self, *};
use crate::value::Value;

pub fn extract_from_value<'a>(value: &'a Value, path: &[&str]) -> &'a Node {
    extract_from_node(&value.0, path)
}

pub fn extract_from_node<'a>(node: &'a Node, path: &[&str]) -> &'a Node {
    match path.first() {
        Some(head) => match node {
            Sub(mapping) => {
                extract_from_node(mapping.get(*head).expect(INVALID_PATH_MSG), &path[1..])
            }
            AnonMap(mapping) => extract_from_node(
                mapping
                    .get(&str::parse(*head).expect(INVALID_PATH_MSG))
                    .expect(INVALID_PATH_MSG),
                &path[1..],
            ),
            Real { .. } | Int { .. } | Bool { .. } => panic!("Invalid path"),
        },
        None => node,
    }
}

static INVALID_PATH_MSG: &str = "Invalid path";

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

        assert_eq!(*extract_from_value(&value, &["foo", "5"]), Int(6));
    }
}
