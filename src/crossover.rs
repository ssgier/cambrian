use crate::meta::CrossoverParams;
use crate::path::{PathContext, PathNodeContext};
use crate::selection::Selection;
use crate::selection::SelectionImpl;
use crate::spec_util::is_leaf;
use crate::{spec, spec::Spec, value, value::Value};
use itertools::Itertools;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand_distr::{Bernoulli, Distribution};
use std::collections::HashMap;
use std::ops::Deref;

impl Default for Crossover<SelectionImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl Crossover<SelectionImpl> {
    pub fn new() -> Self {
        Self {
            selection: SelectionImpl::new(),
        }
    }
}

impl<S> Crossover<S>
where
    S: Selection,
{
    pub fn crossover(
        &self,
        spec: &Spec,
        individuals_ordered: &[&Value],
        crossover_params: &CrossoverParams,
        path_ctx: &mut PathContext,
        rng: &mut StdRng,
    ) -> Value {
        let individuals_ordered: Vec<&value::Node> =
            individuals_ordered.iter().map(|v| &v.0).collect();

        let rescaled_crossover_params = path_ctx
            .0
            .rescaling_ctx
            .current_rescaling
            .rescale_crossover(crossover_params);

        Value(self.do_crossover(
            &spec.0,
            &individuals_ordered,
            &rescaled_crossover_params,
            &mut path_ctx.0,
            rng,
        ))
    }

    fn do_crossover_optional(
        &self,
        spec_node: &spec::Node,
        individuals_ordered: &[Option<&value::Node>],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> Option<value::Node> {
        if individuals_ordered.len() > 1 {
            let mut select_none = || {
                let presence_values: Vec<bool> = individuals_ordered
                    .iter()
                    .map(Option::is_none)
                    .unique()
                    .collect();

                if presence_values.len() == 1 {
                    presence_values[0]
                } else {
                    self.selection
                        .select_value(
                            individuals_ordered,
                            crossover_params.selection_pressure,
                            rng,
                        )
                        .is_none()
                }
            };

            if select_none() {
                None
            } else {
                let individuals_ordered: Vec<&value::Node> =
                    individuals_ordered.iter().filter_map(|v| *v).collect();
                Some(self.do_crossover(
                    spec_node,
                    &individuals_ordered,
                    crossover_params,
                    path_node_ctx,
                    rng,
                ))
            }
        } else {
            individuals_ordered[0].cloned()
        }
    }

    fn do_crossover(
        &self,
        spec_node: &spec::Node,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        if let spec::Node::Const = spec_node {
            return value::Node::Const;
        }

        if individuals_ordered.len() > 1 {
            let mut decide_to_crossover = || {
                Bernoulli::new(crossover_params.crossover_prob)
                    .unwrap()
                    .sample(rng)
            };

            let is_leaf = is_leaf(spec_node);

            if is_leaf || !decide_to_crossover() {
                if is_leaf && are_all_same(individuals_ordered) {
                    let probe = individuals_ordered.iter().next().unwrap();
                    (*probe).clone()
                } else {
                    self.selection
                        .select_ref(
                            individuals_ordered,
                            crossover_params.selection_pressure,
                            rng,
                        )
                        .clone()
                }
            } else {
                match spec_node {
                    spec::Node::Sub { map: spec_map, .. } => self.crossover_sub(
                        spec_map,
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::AnonMap {
                        value_type,
                        min_size,
                        max_size,
                        ..
                    } => self.crossover_anon_map(
                        value_type,
                        (*min_size, *max_size),
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::Variant { map, .. } => self.crossover_variant(
                        map,
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::Optional { value_type, .. } => self.crossover_optional(
                        value_type,
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::Int { .. }
                    | spec::Node::Real { .. }
                    | spec::Node::Bool { .. }
                    | spec::Node::Enum { .. }
                    | spec::Node::Const => {
                        unreachable!()
                    }
                }
            }
        } else {
            individuals_ordered[0].clone()
        }
    }

    fn crossover_sub(
        &self,
        spec_map: &HashMap<String, Box<spec::Node>>,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        let result_value_map: HashMap<String, Box<value::Node>> = spec_map
            .iter()
            .map(|(key, child_spec_node)| {
                let child_values: Vec<&value::Node> = individuals_ordered
                    .iter()
                    .map(|value| {
                        if let value::Node::Sub(value_map) = *value {
                            value_map.get(key).map(|v| v.deref()).unwrap()
                        } else {
                            unreachable!()
                        }
                    })
                    .collect();

                let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(key);

                let child_crossover_params = child_path_node_ctx
                    .rescaling_ctx
                    .current_rescaling
                    .rescale_crossover(crossover_params);

                (
                    key,
                    self.do_crossover(
                        child_spec_node,
                        &child_values,
                        &child_crossover_params,
                        child_path_node_ctx,
                        rng,
                    ),
                )
            })
            .map(|(child_key, child_val)| (child_key.clone(), Box::new(child_val)))
            .collect();

        value::Node::Sub(result_value_map)
    }

    fn select_anon_map_keys(
        &self,
        size_bounds: (Option<usize>, Option<usize>),
        individuals_ordered: &[&value::Node],
        rng: &mut StdRng,
        crossover_params: &CrossoverParams,
    ) -> Vec<usize> {
        let mut all_keys: Vec<usize> = individuals_ordered
            .iter()
            .flat_map(|individual| {
                if let value::Node::AnonMap(mapping) = *individual {
                    mapping.keys()
                } else {
                    unreachable!()
                }
            })
            .unique()
            .copied()
            .collect();

        all_keys.shuffle(rng);

        let mut selected_keys = Vec::new();

        let min_size = size_bounds.0.unwrap_or(0);
        let max_size = size_bounds.1.unwrap_or(all_keys.len());

        for key in all_keys {
            let is_select_key = if selected_keys.len() < min_size {
                true
            } else {
                let presence_values = individuals_ordered
                    .iter()
                    .map(|ind| extract_anon_map_inner(ind).contains_key(&key))
                    .collect_vec();

                if presence_values.iter().unique().count() > 1 {
                    self.selection.select_value(
                        &presence_values,
                        crossover_params.selection_pressure,
                        rng,
                    )
                } else {
                    presence_values[0]
                }
            };

            if is_select_key {
                selected_keys.push(key);
            }

            if selected_keys.len() == max_size {
                break;
            }
        }

        selected_keys
    }

    fn crossover_anon_map(
        &self,
        value_type: &spec::Node,
        size_bounds: (Option<usize>, Option<usize>),
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        let selected_keys =
            self.select_anon_map_keys(size_bounds, individuals_ordered, rng, crossover_params);

        let result_value_map: HashMap<usize, Box<value::Node>> = selected_keys
            .iter()
            .map(|key| {
                let child_values: Vec<&value::Node> = individuals_ordered
                    .iter()
                    .filter_map(|individual| {
                        extract_anon_map_inner(individual)
                            .get(key)
                            .map(Deref::deref)
                    })
                    .collect();

                let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(&key.to_string());

                let child_crossover_params = child_path_node_ctx
                    .rescaling_ctx
                    .current_rescaling
                    .rescale_crossover(crossover_params);

                (
                    *key,
                    self.do_crossover(
                        value_type,
                        &child_values,
                        &child_crossover_params,
                        child_path_node_ctx,
                        rng,
                    ),
                )
            })
            .map(|(child_key, child_val)| (child_key, Box::new(child_val)))
            .collect();

        value::Node::AnonMap(result_value_map)
    }

    fn crossover_variant(
        &self,
        spec_map: &HashMap<String, Box<spec::Node>>,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        let num_distinct_variant_names = individuals_ordered
            .iter()
            .map(|individual| extract_variant_name(individual))
            .unique()
            .count();

        let selected_variant_name = if num_distinct_variant_names > 1 {
            let selected_individual = self.selection.select_ref(
                individuals_ordered,
                crossover_params.selection_pressure,
                rng,
            );

            extract_variant_name(selected_individual)
        } else {
            extract_variant_name(individuals_ordered.iter().next().unwrap())
        };

        let child_spec_node = spec_map.get(selected_variant_name).unwrap();

        let filtered_children_ordered: Vec<&value::Node> = individuals_ordered
            .iter()
            .filter_map(|individual| {
                if let value::Node::Variant(name, value) = individual {
                    if name == selected_variant_name {
                        Some(value.deref())
                    } else {
                        None
                    }
                } else {
                    unreachable!()
                }
            })
            .collect();

        let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(selected_variant_name);

        let child_crossover_params = child_path_node_ctx
            .rescaling_ctx
            .current_rescaling
            .rescale_crossover(crossover_params);

        let child_value = self.do_crossover(
            child_spec_node,
            &filtered_children_ordered,
            &child_crossover_params,
            child_path_node_ctx,
            rng,
        );

        value::Node::Variant(selected_variant_name.to_owned(), Box::new(child_value))
    }

    fn crossover_optional(
        &self,
        value_type: &spec::Node,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        let child_values: Vec<Option<&value::Node>> = individuals_ordered
            .iter()
            .map(|individual| {
                if let value::Node::Optional(value) = *individual {
                    value.as_ref().map(|x| x.deref())
                } else {
                    unreachable!()
                }
            })
            .collect();

        let child_path_node_ctx = path_node_ctx.get_or_create_child_mut("optional");

        let child_crossover_params = child_path_node_ctx
            .rescaling_ctx
            .current_rescaling
            .rescale_crossover(crossover_params);

        value::Node::Optional(
            self.do_crossover_optional(
                value_type,
                &child_values,
                &child_crossover_params,
                child_path_node_ctx,
                rng,
            )
            .map(Box::new),
        )
    }
}

fn are_all_same_helper<F, T>(
    probe: &value::Node,
    individuals_ordered: &[&value::Node],
    value_extractor: F,
) -> bool
where
    F: Fn(&value::Node) -> T,
    T: PartialEq,
{
    let probe_value = value_extractor(probe);

    !individuals_ordered
        .iter()
        .map(|individual| value_extractor(individual))
        .any(|value| value != probe_value)
}

fn are_all_same(individuals_ordered: &[&value::Node]) -> bool {
    let probe = individuals_ordered.iter().next().unwrap();

    match probe {
        value::Node::Real(_) => are_all_same_helper(probe, individuals_ordered, |individual| {
            if let value::Node::Real(value) = individual {
                *value
            } else {
                unreachable!()
            }
        }),
        value::Node::Int(_) => are_all_same_helper(probe, individuals_ordered, |individual| {
            if let value::Node::Int(value) = individual {
                *value
            } else {
                unreachable!()
            }
        }),
        value::Node::Bool(_) => are_all_same_helper(probe, individuals_ordered, |individual| {
            if let value::Node::Bool(value) = individual {
                *value
            } else {
                unreachable!()
            }
        }),
        value::Node::Enum(probe_value) => !individuals_ordered.iter().any(|individual| {
            if let value::Node::Enum(value) = individual {
                value != probe_value
            } else {
                unreachable!()
            }
        }),
        _ => unreachable!(),
    }
}

fn extract_variant_name(node: &value::Node) -> &str {
    if let value::Node::Variant(variant_name, _) = node {
        variant_name
    } else {
        unreachable!()
    }
}
pub struct Crossover<S: Selection = SelectionImpl> {
    selection: S,
}

fn extract_anon_map_inner(value: &value::Node) -> &HashMap<usize, Box<value::Node>> {
    if let value::Node::AnonMap(mapping) = value {
        mapping
    } else {
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::testutil::set_rescaling_at_path;
    use crate::rescaling::{CrossoverRescaling, MutationRescaling, Rescaling};
    use crate::spec_util;
    use crate::testutil::extract_from_value;
    use rand::SeedableRng;
    use std::cell::Cell;
    use std::collections::HashSet;

    struct SelectionMock {
        selected_indexes: Vec<usize>,
        next_pos: Cell<usize>,
    }

    impl SelectionMock {
        fn new(selected_indexes: &[usize]) -> Self {
            Self {
                selected_indexes: Vec::from(selected_indexes),
                next_pos: Cell::new(0),
            }
        }
    }

    impl Selection for SelectionMock {
        fn select_ref<'a, T>(
            &self,
            individuals_ordered: &[&'a T],
            _selection_pressure: f64,
            _rng: &mut StdRng,
        ) -> &'a T {
            if individuals_ordered.len() == 1 {
                individuals_ordered[0]
            } else {
                let pos = self.next_pos.take();
                self.next_pos.set(pos + 1);
                individuals_ordered[self.selected_indexes[pos]]
            }
        }
    }

    struct PressureAwareSelectionMock {}

    impl Selection for PressureAwareSelectionMock {
        fn select_ref<'a, T>(
            &self,
            individuals_ordered: &[&'a T],
            selection_pressure: f64,
            _rng: &mut StdRng,
        ) -> &'a T {
            let target_idx = if selection_pressure > 0.5 { 0 } else { 1 };
            individuals_ordered[target_idx]
        }
    }

    fn make_crossover(selection_indexes: &[usize]) -> Crossover<SelectionMock> {
        Crossover {
            selection: SelectionMock::new(selection_indexes),
        }
    }

    fn make_crossover_with_pressure_aware_selection() -> Crossover<PressureAwareSelectionMock> {
        Crossover {
            selection: PressureAwareSelectionMock {},
        }
    }

    const SELECT_0: f64 = 1.0;
    const SELECT_1: f64 = 0.0;
    fn make_rescaling(crossover_prob_factor: f64, selection_pressure_factor: f64) -> Rescaling {
        Rescaling {
            crossover_rescaling: CrossoverRescaling {
                crossover_prob_factor,
                selection_pressure_factor,
            },
            mutation_rescaling: MutationRescaling::default(),
        }
    }

    fn no_crossover_rescaling() -> Rescaling {
        make_rescaling(0.0, 1.0)
    }

    const ALWAYS_CROSSOVER_PARAMS: CrossoverParams = CrossoverParams {
        crossover_prob: 1.0,
        selection_pressure: 1.0,
    };

    const NEVER_CROSSOVER_PARAMS: CrossoverParams = CrossoverParams {
        crossover_prob: 0.0,
        selection_pressure: 1.0,
    };

    fn make_rng() -> StdRng {
        StdRng::seed_from_u64(0)
    }

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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_real_all_same() {
        let spec_str = "
        type: real
        init: 0
        scale: 1
        ";

        let value0 = Value(value::Node::Real(1.0));
        let value1 = Value(value::Node::Real(1.0));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_int_all_same() {
        let spec_str = "
        type: int
        init: 0
        scale: 1
        ";

        let value0 = Value(value::Node::Int(1));
        let value1 = Value(value::Node::Int(1));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_const() {
        let spec_str = "
        type: const
        ";

        let value0 = Value(value::Node::Const);
        let value1 = Value(value::Node::Const);

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_bool_all_same() {
        let spec_str = "
        type: bool
        init: true
        ";

        let value0 = Value(value::Node::Bool(true));
        let value1 = Value(value::Node::Bool(true));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_enum() {
        let spec_str = "
        type: enum
        init: foo
        values:
        - foo
        - bar
        ";

        let value0 = Value(value::Node::Enum("foo".to_string()));
        let value1 = Value(value::Node::Enum("bar".to_string()));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn leaf_enum_all_same() {
        let spec_str = "
        type: enum
        init: foo
        values:
        - foo
        - bar
        ";

        let value0 = Value(value::Node::Enum("bar".to_string()));
        let value1 = Value(value::Node::Enum("bar".to_string()));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &NEVER_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_foo = extract_from_value(&result, &["foo"]).unwrap();
        let value_bar = extract_from_value(&result, &["bar"]).unwrap();

        assert_ne!(value_foo, value_bar);
    }

    #[test]
    fn crossover_optional_one_absent() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::Optional(Some(Box::new(value::Node::Bool(
            false,
        )))));
        let value1 = Value(value::Node::Optional(None));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn crossover_optional_both_present() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::Optional(Some(Box::new(value::Node::Bool(
            false,
        )))));
        let value1 = Value(value::Node::Optional(Some(Box::new(value::Node::Bool(
            true,
        )))));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn crossover_optional_one_deep() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            foo:
                type: bool
                init: true
            bar:
                type: bool
                init: true
        ";

        let value0 = Value(value::Node::Optional(Some(Box::new(value::Node::Sub(
            HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(false))),
                ("bar".to_string(), Box::new(value::Node::Bool(false))),
            ]),
        )))));

        let value1 = Value(value::Node::Optional(Some(Box::new(value::Node::Sub(
            HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(true))),
                ("bar".to_string(), Box::new(value::Node::Bool(true))),
            ]),
        )))));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(
            &mut root_path_node_ctx,
            &["optional"],
            no_crossover_rescaling(),
        );

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn crossover_variant_one_deep() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            foo:
                type: bool
                init: true
            bar:
                type: bool
                init: true
        bar:
            type: bool
            init: false
        ";

        let value0 = Value(value::Node::Variant(
            "foo".to_string(),
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(false))),
                ("bar".to_string(), Box::new(value::Node::Bool(false))),
            ]))),
        ));

        let value1 = Value(value::Node::Variant(
            "foo".to_string(),
            Box::new(value::Node::Sub(HashMap::from([
                ("foo".to_string(), Box::new(value::Node::Bool(true))),
                ("bar".to_string(), Box::new(value::Node::Bool(true))),
            ]))),
        ));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &["foo"], no_crossover_rescaling());

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(result, value1);
    }

    #[test]
    fn sub_optional() {
        let spec_str = "
        foo:
            type: optional
            initPresent: false
            valueType:
                type: int
                init: 0
                scale: 1
        bar:
            type: optional
            initPresent: true
            valueType:
                type: int
                init: 0
                scale: 1
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([
            ("foo".to_string(), Box::new(value::Node::Optional(None))),
            ("bar".to_string(), Box::new(value::Node::Optional(None))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            (
                "foo".to_string(),
                Box::new(value::Node::Optional(Some(Box::new(value::Node::Int(0))))),
            ),
            (
                "bar".to_string(),
                Box::new(value::Node::Optional(Some(Box::new(value::Node::Int(0))))),
            ),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        if let value::Node::Sub(mapping) = result.0 {
            assert_eq!(mapping.len(), 2);

            let num_present = mapping
                .values()
                .filter_map(|val| {
                    if let value::Node::Optional(value_option) = val.deref() {
                        value_option.as_ref()
                    } else {
                        unreachable!()
                    }
                })
                .count();

            assert_eq!(num_present, 1);
        } else {
            unreachable!();
        }
    }

    #[test]
    fn anon_map_crossover() {
        let spec_str = "
        type: anon map
        initSize: 1
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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value0 = extract_from_value(&result, &["0"]).unwrap();
        let value1 = extract_from_value(&result, &["1"]).unwrap();

        assert_ne!(value0, value1);
    }

    #[test]
    fn anon_map_crossover_size_bounds() {
        let spec_min_size_str = "
        type: anon map
        initSize: 1
        minSize: 1
        valueType:
            type: bool
            init: true
        ";

        let spec_max_size_str = "
        type: anon map
        initSize: 1
        maxSize: 1
        valueType:
            type: bool
            init: true
        ";

        let value0 = Value(value::Node::AnonMap(HashMap::from([(
            0,
            Box::new(value::Node::Bool(false)),
        )])));

        let value1 = Value(value::Node::AnonMap(HashMap::from([(
            1,
            Box::new(value::Node::Bool(true)),
        )])));

        let spec_min_size = spec_util::from_yaml_str(spec_min_size_str).unwrap();
        let spec_max_size = spec_util::from_yaml_str(spec_max_size_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        let mut path_ctx = PathContext(root_path_node_ctx);

        let mut rng = make_rng();
        let sut = Crossover::new();

        let mut sizes_min_size = HashSet::new();
        let mut sizes_max_size = HashSet::new();

        let crossover_params = CrossoverParams {
            crossover_prob: 1.0,
            selection_pressure: 0.5,
        };

        for _ in 0..100 {
            let result_min_size = sut.crossover(
                &spec_min_size,
                &[&value0, &value1],
                &crossover_params,
                &mut path_ctx,
                &mut rng,
            );

            let result_max_size = sut.crossover(
                &spec_max_size,
                &[&value0, &value1],
                &crossover_params,
                &mut path_ctx,
                &mut rng,
            );

            if let (
                value::Node::AnonMap(mapping_min_size),
                value::Node::AnonMap(mapping_max_size),
            ) = (result_min_size.0, result_max_size.0)
            {
                sizes_min_size.insert(mapping_min_size.len());
                sizes_max_size.insert(mapping_max_size.len());
            } else {
                panic!();
            }
        }

        assert_eq!(sizes_min_size, HashSet::from([1, 2]));
        assert_eq!(sizes_max_size, HashSet::from([0, 1]));
    }

    #[test]
    fn anon_map_optional() {
        let spec_str = "
        type: anon map
        initSize: 1
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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        if let value::Node::AnonMap(mapping) = result.0 {
            assert_eq!(mapping.len(), 1);
            assert_eq!(
                *mapping.values().next().unwrap().deref(),
                value::Node::Bool(true)
            );
        } else {
            unreachable!();
        }
    }

    #[test]
    fn variant_crossover_different_variant() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: bool
            init: false
        bar:
            type: int
            init: 0
            scale: 1
        ";

        let value0 = Value(value::Node::Variant(
            "foo".to_string(),
            Box::new(value::Node::Bool(true)),
        ));

        let value1 = Value(value::Node::Variant(
            "bar".to_string(),
            Box::new(value::Node::Int(3)),
        ));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(
            result.0,
            value::Node::Variant("bar".to_string(), Box::new(value::Node::Int(3)))
        );
    }

    #[test]
    fn variant_crossover_same_variant() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: bool
            init: false
        bar:
            type: int
            init: 0
            scale: 1
        ";

        let value0 = Value(value::Node::Variant(
            "foo".to_string(),
            Box::new(value::Node::Bool(false)),
        ));

        let value1 = Value(value::Node::Variant(
            "bar".to_string(),
            Box::new(value::Node::Bool(true)),
        ));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = make_crossover(&[1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        assert_eq!(
            result.0,
            value::Node::Variant("bar".to_string(), Box::new(value::Node::Bool(true)))
        );
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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = make_crossover(&[0]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_foo = extract_from_value(&result, &["foo"]).unwrap();
        let value_bar = extract_from_value(&result, &["bar"]).unwrap();

        assert_eq!(*value_foo, value::Node::Bool(false));
        assert_eq!(*value_bar, value::Node::Bool(false));
    }

    #[test]
    fn one_deep_crossover_by_rescaling() {
        let spec_str = "
        type: anon map
        initSize: 1
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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = make_crossover(&[0]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_foo = extract_from_value(&result, &["0", "foo"]).unwrap();
        let value_bar = extract_from_value(&result, &["0", "bar"]).unwrap();

        assert_eq!(*value_foo, value::Node::Bool(false));
        assert_eq!(*value_bar, value::Node::Bool(false));
    }

    #[test]
    fn full_depth_crossover() {
        let spec_str = "
        type: anon map
        initSize: 1
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

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = make_crossover(&[0, 1]);

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_foo = extract_from_value(&result, &["0", "foo"]).unwrap();
        let value_bar = extract_from_value(&result, &["0", "bar"]).unwrap();

        assert_eq!(*value_foo, *value_bar);
    }

    #[test]
    fn complex_scenario() {
        let spec_str = "
        foo_sub:
            a:
                type: anon map
                initSize: 1
                valueType:
                    type: bool
                    init: false
            b:
                type: optional
                initPresent: false
                valueType:
                    type: int
                    init: 0
                    scale: 1
        foo:
            type: optional
            initPresent: false
            valueType:
                type: int
                init: 0
                scale: 1
        bar:
            type: optional
            initPresent: false
            valueType:
                type: int
                init: 0
                scale: 1
        ";

        let value0 = Value(value::Node::Sub(HashMap::from([
            (
                "foo_sub".to_string(),
                Box::new(value::Node::Sub(HashMap::from([
                    (
                        "a".to_string(),
                        Box::new(value::Node::AnonMap(HashMap::from([
                            (0, Box::new(value::Node::Bool(false))),
                            (1, Box::new(value::Node::Bool(true))),
                        ]))),
                    ),
                    (
                        "b".to_string(),
                        Box::new(value::Node::Optional(Some(Box::new(value::Node::Int(5))))),
                    ),
                ]))),
            ),
            (
                "foo".to_string(),
                Box::new(value::Node::Optional(Some(Box::new(value::Node::Int(3))))),
            ),
            ("bar".to_string(), Box::new(value::Node::Optional(None))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            (
                "foo_sub".to_string(),
                Box::new(value::Node::Sub(HashMap::from([
                    (
                        "a".to_string(),
                        Box::new(value::Node::AnonMap(HashMap::from([
                            (0, Box::new(value::Node::Bool(true))),
                            (1, Box::new(value::Node::Bool(false))),
                        ]))),
                    ),
                    ("b".to_string(), Box::new(value::Node::Optional(None))),
                ]))),
            ),
            ("foo".to_string(), Box::new(value::Node::Optional(None))),
            (
                "bar".to_string(),
                Box::new(value::Node::Optional(Some(Box::new(value::Node::Int(4))))),
            ),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        // do not explicitly prevent crossover, but select presence value of individual one, which
        // is none
        set_rescaling_at_path(
            &mut root_path_node_ctx,
            &["foo_sub", "b"],
            make_rescaling(1.0, SELECT_1),
        );

        // stop crossover here, select individual 1
        set_rescaling_at_path(
            &mut root_path_node_ctx,
            &["foo_sub", "a"],
            make_rescaling(0.0, SELECT_0),
        );

        set_rescaling_at_path(
            &mut root_path_node_ctx,
            &["foo"],
            make_rescaling(1.0, SELECT_0),
        );

        set_rescaling_at_path(
            &mut root_path_node_ctx,
            &["bar"],
            make_rescaling(1.0, SELECT_1),
        );

        let sut = make_crossover_with_pressure_aware_selection();

        let result = sut.crossover(
            &spec,
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_a0 = extract_from_value(&result, &["foo_sub", "a", "0"]).unwrap();
        let value_a1 = extract_from_value(&result, &["foo_sub", "a", "1"]).unwrap();
        let value_b = extract_from_value(&result, &["foo_sub", "b"]).unwrap();
        let value_at_foo = extract_from_value(&result, &["foo", "optional"]).unwrap();
        let value_at_bar = extract_from_value(&result, &["bar", "optional"]).unwrap();

        let foo_sub_value = extract_from_value(&result, &["foo_sub"]).unwrap();
        match foo_sub_value {
            value::Node::Sub(mapping) => assert_eq!(mapping.len(), 2),
            _ => unreachable!(),
        }

        assert_eq!(*value_a0, value::Node::Bool(false));
        assert_eq!(*value_a1, value::Node::Bool(true));
        assert_eq!(*value_b, value::Node::Optional(None));
        assert_eq!(*value_at_foo, value::Node::Int(3));
        assert_eq!(*value_at_bar, value::Node::Int(4));
    }
}
