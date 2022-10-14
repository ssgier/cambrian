use crate::path::{PathContext, PathNodeContext};
use crate::selection::Selection;
use crate::selection::SelectionImpl;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use rand::rngs::StdRng;
use rand_distr::{Bernoulli, Distribution};

use crate::meta::CrossoverParams;
use crate::spec_util;
use crate::{spec, spec::Spec, value, value::Value};

impl<'a> Crossover<'a, SelectionImpl> {
    pub fn new(spec: &'a Spec) -> Self {
        Self {
            spec,
            selection: SelectionImpl::new(),
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
        path_ctx: &mut PathContext,
        rng: &mut StdRng,
    ) -> Value {
        let individuals_ordered: Vec<Option<&value::Node>> =
            individuals_ordered.iter().map(|v| Some(&v.0)).collect();
        Value(
            self.do_crossover(
                spec_util::is_optional(&self.spec.0),
                &self.spec.0,
                &individuals_ordered,
                crossover_params,
                &mut path_ctx.0,
                rng,
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
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> Option<value::Node> {
        if individuals_ordered.len() > 1 {
            let crossover_params = path_node_ctx
                .rescaling_ctx
                .current_rescaling
                .rescale_crossover(crossover_params);

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
                        .select_value(individuals_ordered, &crossover_params, path_node_ctx, rng)
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
                    path_node_ctx,
                    rng,
                ))
            }
        } else {
            individuals_ordered[0].cloned()
        }
    }

    fn do_crossover_value_present(
        &self,
        spec_node: &spec::Node,
        individuals_ordered: &[&value::Node],
        crossover_params: &CrossoverParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        if individuals_ordered.len() > 1 {
            let mut decide_to_crossover = || {
                Bernoulli::new(crossover_params.crossover_prob)
                    .unwrap()
                    .sample(rng)
            };

            if spec_util::is_leaf(spec_node) || !decide_to_crossover() {
                self.selection
                    .select_ref(individuals_ordered, crossover_params, path_node_ctx, rng)
                    .clone()
            } else {
                match spec_node {
                    spec::Node::Sub { map: spec_map, .. } => self.crossover_sub(
                        spec_map,
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::AnonMap { value_type, .. } => self.crossover_anon_map(
                        value_type,
                        individuals_ordered,
                        crossover_params,
                        path_node_ctx,
                        rng,
                    ),
                    spec::Node::Int { .. } | spec::Node::Real { .. } | spec::Node::Bool { .. } => {
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
                let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(key);

                let child_values: Vec<Option<&value::Node>> = individuals_ordered
                    .iter()
                    .map(|value| {
                        if let value::Node::Sub(value_map) = *value {
                            value_map.get(key).map(|v| v.deref())
                        } else {
                            unreachable!()
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
                        child_path_node_ctx,
                        rng,
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
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> value::Node {
        let all_keys: HashSet<&usize> = individuals_ordered
            .iter()
            .flat_map(|individual| {
                if let value::Node::AnonMap(mapping) = *individual {
                    mapping.keys()
                } else {
                    unreachable!()
                }
            })
            .collect();

        let result_value_map: HashMap<usize, Box<value::Node>> = all_keys
            .iter()
            .map(|key| {
                let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(&key.to_string());
                let child_values: Vec<Option<&value::Node>> = individuals_ordered
                    .iter()
                    .map(|individual| {
                        if let value::Node::AnonMap(mapping) = *individual {
                            mapping.get(key).map(Deref::deref)
                        } else {
                            unreachable!()
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
                        child_path_node_ctx,
                        rng,
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

pub struct Crossover<'a, S: Selection = SelectionImpl> {
    spec: &'a Spec,
    selection: S,
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
            _path_node_ctx: &mut PathNodeContext,
            _rng: &mut StdRng,
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

    struct PressureAwareSelectionMock {}

    impl Selection for PressureAwareSelectionMock {
        fn select_ref<'b, T>(
            &self,
            individuals_ordered: &[&'b T],
            crossover_params: &CrossoverParams,
            _path_node_ctx: &mut PathNodeContext,
            _rng: &mut StdRng,
        ) -> &'b T {
            let target_idx = if crossover_params.selection_pressure > 0.5 {
                0
            } else {
                1
            };
            individuals_ordered[target_idx]
        }
    }

    struct TestCrossoverMaker {
        spec: Spec,
    }

    impl TestCrossoverMaker {
        fn from_spec(spec: Spec) -> Self {
            Self { spec }
        }

        fn make<'a>(&'a self, selection_indexes: &'a [usize]) -> Crossover<'a, SelectionMock<'a>> {
            Crossover {
                spec: &self.spec,
                selection: SelectionMock::new(selection_indexes),
            }
        }

        fn make_with_pressure_aware_selection(
            &'_ self,
        ) -> Crossover<'_, PressureAwareSelectionMock> {
            Crossover {
                spec: &self.spec,
                selection: PressureAwareSelectionMock {},
            }
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

        let maker = TestCrossoverMaker::from_spec(spec);

        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[1]);

        let result = sut.crossover(
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

        let maker = TestCrossoverMaker::from_spec(spec);
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[1]);

        let result = sut.crossover(
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

        let maker = TestCrossoverMaker::from_spec(spec);
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[1]);

        let result = sut.crossover(
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

        let maker = TestCrossoverMaker::from_spec(spec);
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
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

        let maker = TestCrossoverMaker::from_spec(spec);
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        if let value::Node::Sub(mapping) = result.0 {
            assert_eq!(mapping.len(), 1);
            assert_eq!(
                *mapping.values().next().unwrap().deref(),
                value::Node::Int(0)
            );
        } else {
            unreachable!();
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = maker.make(&[0]);

        let result = sut.crossover(
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);
        set_rescaling_at_path(&mut root_path_node_ctx, &[], no_crossover_rescaling());

        let sut = maker.make(&[0]);

        let result = sut.crossover(
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
        let mut root_path_node_ctx = PathNodeContext::default();
        root_path_node_ctx.add_nodes_for(&value0.0);
        root_path_node_ctx.add_nodes_for(&value1.0);

        let sut = maker.make(&[0, 1]);

        let result = sut.crossover(
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
                valueType:
                    type: bool
            b:
                type: int
                optional: true
                init: 0
                scale: 1
        foo:
            type: int
            optional: true
            init: 0
            scale: 1
        bar:
            type: int
            optional: true
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
                    ("b".to_string(), Box::new(value::Node::Int(5))),
                ]))),
            ),
            ("foo".to_string(), Box::new(value::Node::Int(3))),
        ])));

        let value1 = Value(value::Node::Sub(HashMap::from([
            (
                "foo_sub".to_string(),
                Box::new(value::Node::Sub(HashMap::from([(
                    "a".to_string(),
                    Box::new(value::Node::AnonMap(HashMap::from([
                        (0, Box::new(value::Node::Bool(true))),
                        (1, Box::new(value::Node::Bool(false))),
                    ]))),
                )]))),
            ),
            ("bar".to_string(), Box::new(value::Node::Int(4))),
        ])));

        let spec = spec_util::from_yaml_str(spec_str).unwrap();

        let maker = TestCrossoverMaker::from_spec(spec);
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

        let sut = maker.make_with_pressure_aware_selection();

        let result = sut.crossover(
            &[&value0, &value1],
            &ALWAYS_CROSSOVER_PARAMS,
            &mut PathContext(root_path_node_ctx),
            &mut make_rng(),
        );

        let value_a0 = extract_from_value(&result, &["foo_sub", "a", "0"]).unwrap();
        let value_a1 = extract_from_value(&result, &["foo_sub", "a", "1"]).unwrap();
        let value_at_foo = extract_from_value(&result, &["foo"]).unwrap();
        let value_at_bar = extract_from_value(&result, &["bar"]).unwrap();

        let foo_sub_value = extract_from_value(&result, &["foo_sub"]).unwrap();
        match foo_sub_value {
            value::Node::Sub(mapping) => assert_eq!(mapping.len(), 1),
            _ => unreachable!(),
        }

        assert_eq!(*value_a0, value::Node::Bool(false));
        assert_eq!(*value_a1, value::Node::Bool(true));
        assert_eq!(*value_at_foo, value::Node::Int(3));
        assert_eq!(*value_at_bar, value::Node::Int(4));
    }
}
