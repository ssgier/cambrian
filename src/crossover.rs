use crate::path::{PathManager, PathNode};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use rand::rngs::StdRng;
use rand_distr::{Bernoulli, Distribution};

use crate::meta::CrossoverParams;
use crate::rescaling::RescalingManager;
use crate::spec_util;
use crate::{spec, spec::Spec, value, value::Value};

impl<'a> Crossover<'a, SelectionImpl<'a>> {
    pub fn new(
        path_manager: &'a RefCell<PathManager>,
        rescaling_manager: &'a RefCell<RescalingManager>,
        spec: &'a Spec,
        rng: &'a RefCell<StdRng>,
    ) -> Self {
        Self {
            path_manager,
            rescaling_manager,
            spec,
            rng,
            selection: SelectionImpl { _rng: rng },
        }
    }

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
        let crossover_params = self
            .rescaling_manager
            .borrow()
            .get_for_path_node(path_node)
            .rescale_crossover(crossover_params);

        let select_none = || {
            self.selection
                .select_value(individuals_ordered, &crossover_params)
                .is_none()
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
                    .filter(Option::is_some)
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
    rescaling_manager: &'a RefCell<RescalingManager>,
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
        _individuals_ordered: &[&T],
        _crossover_params: &CrossoverParams,
    ) -> T {
        panic!("not implemented"); // TODO, adaptive params here as well
    }
}

pub trait Selection {
    fn select_ref<T>(&self, individuals_ordered: &[&T], crossover_params: &CrossoverParams) -> T;

    fn select_value<T>(&self, individuals_ordered: &[T], crossover_params: &CrossoverParams) -> T {
        let individuals_ordered: Vec<&T> = individuals_ordered.iter().collect();
        self.select_ref(&individuals_ordered, crossover_params)
    }
}
