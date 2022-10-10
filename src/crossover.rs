use crate::path::{PathManager, PathNode};
use itertools::Itertools;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use rand::rngs::StdRng;
use rand_distr::{Bernoulli, Distribution};

use crate::meta::CrossoverParams;
use crate::spec_util;
use crate::{spec, spec::Spec, value, value::Value};

impl<'a> Crossover<'a, SelectionImpl<'a>> {
    pub fn new(
        path_manager: &'a RefCell<PathManager>,
        spec: &'a Spec,
        rng: &'a RefCell<StdRng>,
    ) -> Self {
        Self {
            path_manager,
            spec,
            rng,
            selection: SelectionImpl { _rng: rng },
        }
    }
}

impl<'a, S> Crossover<'a, S>
where
    S: Selection,
{
    pub fn crossover(
        &self,
        individuals_ordered: &[&Value],
        crossover_params: &CrossoverParams,
    ) -> Value {
        let individuals_ordered: Vec<Option<&value::Node>> =
            individuals_ordered.iter().map(|v| Some(&v.0)).collect();
        Value(
            self.do_crossover(
                spec_util::is_optional(&self.spec.0),
                &self.spec.0,
                &individuals_ordered,
                crossover_params,
                self.path_manager.borrow().root(),
            )
            .unwrap(),
        )
    }

    fn do_crossover(
        &self,
        is_optional: bool,
        spec_node: &spec::Node,
        individuals_ordered: &[Option<&value::Node>],
        crossover_params: &CrossoverParams,
        path_node: &PathNode,
    ) -> Option<value::Node> {
        let crossover_params = path_node
            .rescaling_ctx
            .current_rescaling
            .rescale_crossover(crossover_params);

        let select_none = || {
            let presence_values: Vec<bool> = individuals_ordered
                .iter()
                .map(Option::is_none)
                .unique()
                .collect();

            if presence_values.len() == 1 {
                presence_values[0]
            } else {
                self.selection
                    .select_value(individuals_ordered, &crossover_params)
                    .is_none()
            }
        };

        if is_optional && select_none() {
            None
        } else {
            let individuals_ordered: Vec<&value::Node> =
                individuals_ordered.iter().filter_map(|v| *v).collect();
            Some(self.do_crossover_value_present(
                spec_node,
                &individuals_ordered,
                &crossover_params,
                path_node,
            ))
        }
    }

    fn do_crossover_value_present(
        &self,
        spec_node: &spec::Node,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node: &PathNode,
    ) -> value::Node {
        let decide_to_crossover = || {
            Bernoulli::new(crossover_params.crossover_prob)
                .unwrap()
                .sample(&mut *self.rng.borrow_mut())
        };

        if spec_util::is_leaf(spec_node) || !decide_to_crossover() {
            self.selection
                .select_ref(individuals_ordered, crossover_params)
                .clone()
        } else {
            match spec_node {
                spec::Node::Sub { map: spec_map, .. } => {
                    self.crossover_sub(spec_map, individuals_ordered, crossover_params, path_node)
                }
                spec::Node::AnonMap { value_type, .. } => self.crossover_anon_map(
                    value_type,
                    individuals_ordered,
                    crossover_params,
                    path_node,
                ),
                spec::Node::Int { .. } | spec::Node::Real { .. } | spec::Node::Bool { .. } => {
                    panic!()
                }
            }
        }
    }

    fn crossover_sub(
        &self,
        spec_map: &HashMap<String, Box<spec::Node>>,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node: &PathNode,
    ) -> value::Node {
        let result_value_map: HashMap<String, Box<value::Node>> = spec_map
            .iter()
            .map(|(key, child_spec_node)| {
                let path_manager = self.path_manager.borrow();
                let child_path_node = path_manager.child_of(path_node, key);

                let child_values: Vec<Option<&value::Node>> = individuals_ordered
                    .iter()
                    .map(|value| {
                        if let value::Node::Sub(value_map) = *value {
                            value_map.get(key).map(|v| v.deref())
                        } else {
                            panic!("bad state")
                        }
                    })
                    .collect();

                (
                    key,
                    self.do_crossover(
                        spec_util::is_optional(child_spec_node),
                        child_spec_node,
                        &child_values,
                        crossover_params,
                        child_path_node,
                    ),
                )
            })
            .filter_map(|(child_key, child_val)| {
                child_val.map(|present_value| (child_key.clone(), Box::new(present_value)))
            })
            .collect();

        value::Node::Sub(result_value_map)
    }

    fn crossover_anon_map(
        &self,
        value_type: &spec::Node,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node: &PathNode,
    ) -> value::Node {
        let all_keys: HashSet<&usize> = individuals_ordered
            .iter()
            .flat_map(|individual| {
                if let value::Node::AnonMap(mapping) = *individual {
                    mapping.keys()
                } else {
                    panic!("bad state")
                }
            })
            .collect();

        let result_value_map: HashMap<usize, Box<value::Node>> = all_keys
            .iter()
            .map(|key| {
                let path_manager = self.path_manager.borrow();
                let child_path_node = path_manager.child_of(path_node, &key.to_string());
                let child_values: Vec<Option<&value::Node>> = individuals_ordered
                    .iter()
                    .map(|individual| {
                        if let value::Node::AnonMap(mapping) = *individual {
                            mapping.get(key).map(Deref::deref)
                        } else {
                            panic!("bad state")
                        }
                    })
                    .collect();

                (
                    *key,
                    self.do_crossover(
                        true,
                        value_type,
                        &child_values,
                        crossover_params,
                        child_path_node,
                    ),
                )
            })
            .filter_map(|(child_key, child_val)| {
                child_val.map(|present_value| (*child_key, Box::new(present_value)))
            })
            .collect();

        value::Node::AnonMap(result_value_map)
    }
}

pub struct Crossover<'a, S: Selection = SelectionImpl<'a>> {
    path_manager: &'a RefCell<PathManager>,
    spec: &'a Spec,
    rng: &'a RefCell<StdRng>,
    selection: S,
}

pub struct SelectionImpl<'a> {
    _rng: &'a RefCell<StdRng>,
}

impl Selection for SelectionImpl<'_> {
    fn select_ref<'a, T>(
        &self,
        _individuals_ordered: &[&'a T],
        _crossover_params: &CrossoverParams,
    ) -> &'a T {
        panic!("not implemented"); // TODO, adaptive params here as well
    }
}

pub trait Selection {
    fn select_ref<'a, T>(
        &self,
        individuals_ordered: &[&'a T],
        crossover_params: &CrossoverParams,
    ) -> &'a T;

    fn select_value<T: Clone>(
        &self,
        individuals_ordered: &[T],
        crossover_params: &CrossoverParams,
    ) -> T {
        let individuals_ordered: Vec<&T> = individuals_ordered.iter().collect();
        self.select_ref(&individuals_ordered, crossover_params)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::path::testutil::set_rescaling_at_path;
    use crate::rescaling::{CrossoverRescaling, MutationRescaling, Rescaling};
    use crate::testutil::extract_from_value;

    use super::*;
    use rand::SeedableRng;
    use std::cell::Cell;

    struct SelectionMock<'a> {
        selected_indexes: &'a [usize],
        next_pos: Cell<usize>,
    }

    impl<'a> SelectionMock<'a> {
        fn new(selected_indexes: &'a [usize]) -> Self {
            Self {
                selected_indexes,
                next_pos: Cell::new(0),
            }
        }
    }

    impl<'a> Selection for SelectionMock<'a> {
        fn select_ref<'b, T>(
            &self,
            individuals_ordered: &[&'b T],
            _crossover_params: &CrossoverParams,
        ) -> &'b T {
            if individuals_ordered.len() == 1 {
                individuals_ordered[0]
            } else {
                let pos = self.next_pos.take();
                self.next_pos.set(pos + 1);
                individuals_ordered[self.selected_indexes[pos]]
            }
        }
    }

    struct TestCrossoverMaker {
        path_manager: RefCell<PathManager>,
        spec: Spec,
        rng: RefCell<StdRng>,
    }

    impl TestCrossoverMaker {
        fn from_spec(spec: Spec) -> Self {
            Self {
                path_manager: RefCell::new(PathManager::new()),
                spec,
                rng: RefCell::new(StdRng::seed_from_u64(0)),
            }
        }

        fn make<'a>(&'a self, selection_indexes: &'a [usize]) -> Crossover<'a, SelectionMock<'a>> {
            Crossover {
                path_manager: &self.path_manager,
                spec: &self.spec,
                rng: &self.rng,
                selection: SelectionMock::new(selection_indexes),
            }
        }
    }

    fn no_crossover_rescaling() -> Rescaling {
        Rescaling {
            crossover_rescaling: CrossoverRescaling {
                crossover_prob_factor: 0.0,
            },
            mutation_rescaling: MutationRescaling::default(),
        }
    }

    const ALWAYS_CROSSOVER_PARAMS: CrossoverParams = CrossoverParams {
        crossover_prob: 1.0,
    };

    const NEVER_CROSSOVER_PARAMS: CrossoverParams = CrossoverParams {
        crossover_prob: 0.0,
    };

    #[test]
    fn leaf_real() {
        let spec_str = "
        type: real
        init: 0
        scale: 1
        ";

        let value0 = Value(value::Node::Real(0.0));
        let value1 = Value(value::Node::Real(1.0));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_int() {
        let spec_str = "
        type: int
        init: 0
        scale: 1
        ";

        let value0 = Value(value::Node::Int(0));
        let value1 = Value(value::Node::Int(1));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_bool() {
        let spec_str = "
        type: bool
        init: true
        ";

        let value0 = Value(value::Node::Bool(false));
        let value1 = Value(value::Node::Bool(true));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        assert_eq!(result, value1);
    }

    #[test]
    fn sub_no_crossover() {
        let spec_str = "
        foo:
            type: bool
            init: true
        bar:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(false))),
            ("bar".to_string(), Box::new(value::Node::Bool(false))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(true))),
            ("bar".to_string(), Box::new(value::Node::Bool(true))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &NEVER_CROSSOVER_PARAMS);
        assert_eq!(result, value0);
    }

    #[test]
    fn sub_crossover() {
        let spec_str = "
        foo:
            type: bool
            init: true
        bar:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(false))),
            ("bar".to_string(), Box::new(value::Node::Bool(false))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(true))),
            ("bar".to_string(), Box::new(value::Node::Bool(true))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);

        let value_foo = extract_from_value(&result, &["foo"]);
        let value_bar = extract_from_value(&result, &["bar"]);

        assert_ne!(value_foo, value_bar);
    }

    #[test]
    fn sub_optional() {
        let spec_str = "
        foo:
            type: int
            init: 0
            scale: 1
            optional: true
        bar:
            type: int
            init: 0
            scale: 1
            optional: true
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Int(0))),
            ("bar".to_string(), Box::new(value::Node::Int(0))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);

        if let value::Node::Sub(mapping) = result.0 {
            assert_eq!(mapping.len(), 1);
            assert_eq!(
                *mapping.values().next().unwrap().deref(),
                value::Node::Int(0)
            );
        } else {
            panic!();
        }
    }

    #[test]
    fn anon_map_crossover() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::AnonMap(HashMap::from([
            (0, Box::new(value::Node::Bool(false))),
            (1, Box::new(value::Node::Bool(false))),
        ])));

        let value1 = Value(value::Node::AnonMap(HashMap::from([
            (0, Box::new(value::Node::Bool(true))),
            (1, Box::new(value::Node::Bool(true))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);

        let value0 = extract_from_value(&result, &["0"]);
        let value1 = extract_from_value(&result, &["1"]);

        assert_ne!(value0, value1);
    }

    #[test]
    fn anon_map_optional() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::AnonMap(HashMap::from([])));

        let value1 = Value(value::Node::AnonMap(HashMap::from([
            (0, Box::new(value::Node::Bool(true))),
            (1, Box::new(value::Node::Bool(true))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);

        if let value::Node::AnonMap(mapping) = result.0 {
            assert_eq!(mapping.len(), 1);
            assert_eq!(
                *mapping.values().next().unwrap().deref(),
                value::Node::Bool(true)
            );
        } else {
            panic!();
        }
    }

    #[test]
    fn rescaling_at_root() {
        let spec_str = "
        foo:
            type: bool
            init: true
        bar:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(false))),
            ("bar".to_string(), Box::new(value::Node::Bool(false))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Bool(true))),
            ("bar".to_string(), Box::new(value::Node::Bool(true))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        set_rescaling_at_path(
            &mut maker.path_manager.borrow_mut(),
            &[],
            no_crossover_rescaling(),
        );

        let sut = maker.make(&[0]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        let value_foo = extract_from_value(&result, &["foo"]);
        let value_bar = extract_from_value(&result, &["bar"]);

        assert_eq!(*value_foo, value::Node::Bool(false));
        assert_eq!(*value_bar, value::Node::Bool(false));
    }

    #[test]
    fn one_deep_crossover_by_rescaling() {
        let spec_str = "
        type: anon map
        valueType:
            foo:
                type: bool
                init: true
            bar:
                type: bool
                init: true
        ";

        let value0 = Value(value::Node::AnonMap(HashMap::from([(
            0,
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(false))),
                ("bar".to_string(), Box::new(value::Node::Bool(false))),
            ]))),
        )])));

        let value1 = Value(value::Node::AnonMap(HashMap::from([(
            0,
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(true))),
                ("bar".to_string(), Box::new(value::Node::Bool(true))),
            ]))),
        )])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        set_rescaling_at_path(
            &mut maker.path_manager.borrow_mut(),
            &[],
            no_crossover_rescaling(),
        );

        let sut = maker.make(&[0]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        let value_foo = extract_from_value(&result, &["0", "foo"]);
        let value_bar = extract_from_value(&result, &["0", "bar"]);

        assert_eq!(*value_foo, value::Node::Bool(false));
        assert_eq!(*value_bar, value::Node::Bool(false));
    }

    #[test]
    fn full_depth_crossover() {
        let spec_str = "
        type: anon map
        valueType:
            foo:
                type: bool
                init: true
            bar:
                type: bool
                init: true
        ";

        let value0 = Value(value::Node::AnonMap(HashMap::from([(
            0,
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(false))),
                ("bar".to_string(), Box::new(value::Node::Bool(true))),
            ]))),
        )])));

        let value1 = Value(value::Node::AnonMap(HashMap::from([(
            0,
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(true))),
                ("bar".to_string(), Box::new(value::Node::Bool(false))),
            ]))),
        )])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
        maker.path_manager.borrow_mut().add_all_nodes(&value0);
        maker.path_manager.borrow_mut().add_all_nodes(&value1);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(&[&value0, &value1], &ALWAYS_CROSSOVER_PARAMS);
        let value_foo = extract_from_value(&result, &["0", "foo"]);
        let value_bar = extract_from_value(&result, &["0", "bar"]);

        assert_eq!(*value_foo, *value_bar);
    }
}
