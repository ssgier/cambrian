use crate::rescaling::RescalingContext;
use crate::value;
use crate::value::Node::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct PathContext(pub PathNodeContext);

#[derive(Default)]
pub struct PathNodeContext {
    child_nodes: HashMap<String, Box<PathNodeContext>>,
    pub rescaling_ctx: RescalingContext,
    id_mgr: KeyManager,
}

#[derive(Default)]
pub struct KeyManager {
    next_key: usize,
}

impl KeyManager {
    pub fn next_key(&mut self) -> usize {
        let result = self.next_key;
        self.next_key += 1;
        result
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
                    let child_node = self.child_nodes.entry(key.to_string()).or_default();
                    child_node.add_nodes_for(value);
                }
            }
            Bool { .. } | Int { .. } | Real { .. } => (),
        }
    }

    pub fn get_child(&self, key: &str) -> &PathNodeContext {
        self.child_nodes.get(key).unwrap()
    }

    pub fn get_child_mut(&mut self, key: &str) -> &mut PathNodeContext {
        self.child_nodes.get_mut(key).unwrap()
    }

    pub fn next_key(&mut self) -> usize {
        self.id_mgr.next_key()
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
                "foo".to_string(),
                Box::new(value::Node::AnonMap(HashMap::from([(
                    4,
                    Box::new(value::Node::Real(10.0)),
                )]))),
            ),
        ])));

        let mut sut = PathNodeContext::default();
        sut.add_nodes_for(&value.0);

        assert_eq!(sut.child_nodes.len(), 4);
        for key in ["a", "b", "c"] {
            let node = sut.get_child(key);
            assert!(node.child_nodes.is_empty());
        }

        let foo = sut.get_child("foo");
        assert_eq!(foo.child_nodes.len(), 1);
        let foo_child = foo.get_child("4");
        assert!(foo_child.child_nodes.is_empty());
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
