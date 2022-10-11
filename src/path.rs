use crate::rescaling::RescalingContext;
use crate::value;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PathNode {
    pub id: usize,
    child_ids_by_key: HashMap<String, usize>,
    pub rescaling_ctx: RescalingContext,
}

impl PathNode {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            child_ids_by_key: HashMap::default(),
            rescaling_ctx: RescalingContext::default(),
        }
    }
}

#[derive(Debug)]
pub struct PathManager {
    nodes_by_id: HashMap<usize, PathNode>,
    next_id: usize,
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PathManager {
    pub fn new() -> Self {
        Self {
            nodes_by_id: HashMap::from([(0, PathNode::new(0))]),
            next_id: 1,
        }
    }

    pub fn root(&self) -> &PathNode {
        self.nodes_by_id.get(&0).unwrap()
    }

    fn child_id_of(&self, node_id: usize, key: &str) -> usize {
        *self
            .nodes_by_id
            .get(&node_id)
            .unwrap()
            .child_ids_by_key
            .get(key)
            .unwrap()
    }

    pub fn child_of(&self, path_node: &PathNode, key: &str) -> &PathNode {
        self.child_of_by_id(path_node.id, key)
    }

    pub fn child_of_by_id(&self, node_id: usize, key: &str) -> &PathNode {
        self.nodes_by_id
            .get(&self.child_id_of(node_id, key))
            .unwrap()
    }

    pub fn add_node(&mut self, parent_id: usize, key: &str) -> &PathNode {
        let id = self.next_id;
        self.next_id += 1;
        let new_node = PathNode::new(id);
        let parent = self.nodes_by_id.get_mut(&parent_id).unwrap();
        parent.child_ids_by_key.insert(key.to_string(), id);
        self.nodes_by_id.entry(id).or_insert(new_node)
    }

    pub fn add_all_nodes(&mut self, value: &value::Value) {
        self.add_all_nodes_at(self.root().id, &value.0);
    }

    pub fn add_all_nodes_at(&mut self, parent_node_id: usize, value_node: &value::Node) {
        match value_node {
            value::Node::Sub(mapping) => {
                for (key, child_value_node) in mapping {
                    let child_node_id = match self
                        .nodes_by_id
                        .get_mut(&parent_node_id)
                        .unwrap()
                        .child_ids_by_key
                        .get(key)
                    {
                        Some(child_node_id) => *child_node_id,
                        None => self.add_node(parent_node_id, key).id,
                    };

                    self.add_all_nodes_at(child_node_id, child_value_node);
                }
            }
            value::Node::AnonMap(mapping) => {
                for (key, child_value_node) in mapping {
                    let key_str = key.to_string();

                    let child_node_id = match self
                        .nodes_by_id
                        .get_mut(&parent_node_id)
                        .unwrap()
                        .child_ids_by_key
                        .get(&key_str)
                    {
                        Some(child_node_id) => *child_node_id,
                        None => self.add_node(parent_node_id, &key_str).id,
                    };

                    self.add_all_nodes_at(child_node_id, child_value_node);
                }
            }
            value::Node::Real(_) | value::Node::Int(_) | value::Node::Bool(_) => (),
        }
    }
}

#[cfg(test)]
pub mod testutil {
    use std::usize;

    use crate::rescaling::Rescaling;

    use super::*;
    pub fn set_rescaling_at_path(path_mgr: &mut PathManager, path: &[&str], rescaling: Rescaling) {
        set_rescaling_at_path_relative(path_mgr, path_mgr.root().id, path, rescaling);
    }

    fn set_rescaling_at_path_relative(
        path_mgr: &mut PathManager,
        from_node_id: usize,
        path: &[&str],
        rescaling: Rescaling,
    ) {
        match path.first() {
            Some(head) => {
                let node = path_mgr.nodes_by_id.get(&from_node_id).unwrap();
                set_rescaling_at_path_relative(
                    path_mgr,
                    *node.child_ids_by_key.get(*head).expect("Invalid path"),
                    &path[1..],
                    rescaling,
                );
            }
            None => {
                let node = path_mgr.nodes_by_id.get_mut(&from_node_id).unwrap();
                node.rescaling_ctx.current_rescaling = rescaling;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root() {
        let sut = PathManager::new();
        assert_eq!(sut.root().id, 0);
    }

    #[test]
    fn add_node() {
        let mut sut = PathManager::new();
        let root_id = sut.root().id;
        let child_id = sut.add_node(root_id, "foo").id;
        assert_eq!(sut.child_of_by_id(root_id, "foo").id, child_id);
    }

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

        let mut sut = PathManager::new();
        sut.add_all_nodes(&value);

        assert_eq!(sut.nodes_by_id.len(), 6);
        for key in ["a", "b", "c"] {
            let node = sut.child_of_by_id(sut.root().id, key);
            assert!(node.child_ids_by_key.is_empty());
        }

        let foo = sut.child_of_by_id(sut.root().id, "foo");
        assert_eq!(foo.child_ids_by_key.len(), 1);
        assert!(foo.child_ids_by_key.contains_key("4"));

        let foo_child = sut.child_of(foo, "4");
        assert!(foo_child.child_ids_by_key.is_empty());
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

        let mut sut = PathManager::new();
        sut.add_all_nodes(&value0);
        sut.add_all_nodes(&value1);

        assert_eq!(sut.nodes_by_id.len(), 4);

        let foo = sut.child_of_by_id(sut.root().id, "foo");
        assert_eq!(foo.child_ids_by_key.len(), 2);
        for key in ["4", "5"] {
            assert!(foo.child_ids_by_key.contains_key(key));
            let foo_child = sut.child_of(foo, key);
            assert!(foo_child.child_ids_by_key.is_empty());
        }
    }
}
