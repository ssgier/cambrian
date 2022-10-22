use crate::rescaling::RescalingContext;
use crate::value;
use crate::value::Node::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct PathContext(pub PathNodeContext);

#[derive(Default, Debug)]
pub struct PathNodeContext {
    child_nodes: HashMap<String, Box<PathNodeContext>>,
    pub rescaling_ctx: RescalingContext,
    key_mgr: KeyManager,
}

#[derive(Default, Debug)]
pub struct KeyManager {
    next_key: usize,
}

impl KeyManager {
    pub fn next_key(&mut self) -> usize {
        let result = self.next_key;
        self.next_key += 1;
        result
    }

    fn on_key_seen(&mut self, key: usize) {
        self.next_key = self.next_key.max(key + 1);
    }
}

impl PathNodeContext {
    pub fn add_nodes_for(&mut self, node: &value::Node) {
        match node {
            Sub(mapping) => {
                for (key, value) in mapping {
                    let child_node = self.child_nodes.entry(key.clone()).or_default();
                    child_node.add_nodes_for(value);
                }
            }
            AnonMap(mapping) => {
                for (key, value) in mapping {
                    self.key_mgr.on_key_seen(*key);
                    let child_node = self.child_nodes.entry(key.to_string()).or_default();
                    child_node.add_nodes_for(value);
                }
            }
            Variant(variant_name, value) => {
                let child_node = self.child_nodes.entry(variant_name.clone()).or_default();
                child_node.add_nodes_for(value);
            }
            Optional(value_option) => {
                if let Some(value) = value_option {
                    let child_node = self.child_nodes.entry("optional".to_string()).or_default();
                    child_node.add_nodes_for(value);
                }
            }
            Bool { .. } | Int { .. } | Real { .. } | Enum(_) => (),
        }
    }

    pub fn get_child(&self, key: &str) -> &PathNodeContext {
        self.child_nodes.get(key).unwrap()
    }

    pub fn get_or_create_child_mut(&mut self, key: &str) -> &mut PathNodeContext {
        self.child_nodes.entry(key.to_string()).or_default()
    }

    pub fn next_key(&mut self) -> usize {
        self.key_mgr.next_key()
    }
}

#[cfg(test)]
pub mod testutil {

    use crate::rescaling::Rescaling;

    use super::*;
    pub fn set_rescaling_at_path(
        path_node_ctx: &mut PathNodeContext,
        path: &[&str],
        rescaling: Rescaling,
    ) {
        match path.first() {
            Some(head) => set_rescaling_at_path(
                path_node_ctx.child_nodes.get_mut(*head).unwrap(),
                &path[1..],
                rescaling,
            ),
            None => path_node_ctx.rescaling_ctx.current_rescaling = rescaling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_all_nodes() {
        let value = value::Value(value::Node::Sub(HashMap::from([
            ("a".to_string(), Box::new(value::Node::Real(1.0))),
            ("b".to_string(), Box::new(value::Node::Int(4))),
            ("c".to_string(), Box::new(value::Node::Bool(false))),
            (
                "d".to_string(),
                Box::new(value::Node::Enum("foo".to_string())),
            ),
            (
                "foo".to_string(),
                Box::new(value::Node::AnonMap(HashMap::from([(
                    4,
                    Box::new(value::Node::Real(10.0)),
                )]))),
            ),
            (
                "bar".to_string(),
                Box::new(value::Node::Variant(
                    "bar".to_string(),
                    Box::new(value::Node::Bool(true)),
                )),
            ),
            (
                "optional_foo".to_string(),
                Box::new(value::Node::Optional(Some(Box::new(value::Node::Bool(
                    false,
                ))))),
            ),
        ])));

        let mut sut = PathNodeContext::default();
        sut.add_nodes_for(&value.0);

        assert_eq!(sut.child_nodes.len(), 7);
        for key in ["a", "b", "c", "d"] {
            let node = sut.get_child(key);
            assert!(node.child_nodes.is_empty());
        }

        let foo = sut.get_child("foo");
        assert_eq!(foo.child_nodes.len(), 1);
        let foo_child = foo.get_child("4");
        assert!(foo_child.child_nodes.is_empty());

        let bar = sut.get_child("bar");
        assert_eq!(bar.child_nodes.len(), 1);
        let bar_child = bar.get_child("bar");
        assert!(bar_child.child_nodes.is_empty());

        let optional_foo = sut.get_child("optional_foo");
        assert_eq!(optional_foo.child_nodes.len(), 1);
        let optional_foo_child = optional_foo.get_child("optional");
        assert!(optional_foo_child.child_nodes.is_empty());
    }

    #[test]
    fn partially_overlapping_paths() {
        let value0 = value::Value(value::Node::Sub(HashMap::from([(
            "foo".to_string(),
            Box::new(value::Node::AnonMap(HashMap::from([(
                4,
                Box::new(value::Node::Real(10.0)),
            )]))),
        )])));

        let value1 = value::Value(value::Node::Sub(HashMap::from([(
            "foo".to_string(),
            Box::new(value::Node::AnonMap(HashMap::from([(
                5,
                Box::new(value::Node::Real(10.0)),
            )]))),
        )])));

        let mut sut = PathNodeContext::default();
        sut.add_nodes_for(&value0.0);
        sut.add_nodes_for(&value1.0);

        assert_eq!(sut.child_nodes.len(), 1);

        let foo = sut.get_child("foo");
        assert_eq!(foo.child_nodes.len(), 2);
        for key in ["4", "5"] {
            let foo_child = foo.get_child(key);
            assert!(foo_child.child_nodes.is_empty());
        }
    }
}
