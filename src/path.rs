use std::collections::HashMap;

pub struct PathNode {
    pub id: usize,
    child_ids_by_key: HashMap<String, usize>,
}

pub struct PathManager {
    root_id: usize,
    nodes_by_id: HashMap<usize, PathNode>,
    _next_id: usize,
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PathManager {
    pub fn new() -> Self {
        Self {
            root_id: 0,
            nodes_by_id: HashMap::new(),
            _next_id: 1,
        }
    }

    pub fn root(&self) -> &PathNode {
        self.nodes_by_id.get(&self.root_id).unwrap()
    }

    pub fn child_of(&self, path_node: &PathNode, key: &str) -> &PathNode {
        let child_id = self
            .nodes_by_id
            .get(&path_node.id)
            .unwrap()
            .child_ids_by_key
            .get(key)
            .unwrap();

        self.nodes_by_id.get(child_id).unwrap()
    }
}
