use crate::meta::MutationParams;
use crate::path::{PathContext, PathNodeContext};
use rand::rngs::StdRng;
use rand::seq::IteratorRandom;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Bernoulli, Cauchy, Distribution};
use std::collections::HashMap;

use crate::value;
use crate::value::Value;
use crate::{spec, spec_util};
use lazy_static::lazy_static;

pub fn mutate(
    spec: &spec::Spec,
    individual: &Value,
    mutation_params: &MutationParams,
    path_ctx: &mut PathContext,
    rng: &mut StdRng,
) -> Value {
    let spec_node = &spec.0;
    Value(
        do_mutate(
            Some(&individual.0),
            spec_node,
            spec_util::is_optional(spec_node),
            mutation_params,
            &mut path_ctx.0,
            rng,
        )
        .unwrap(),
    )
}

fn do_mutate(
    value: Option<&value::Node>,
    spec_node: &spec::Node,
    is_optional: bool,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> Option<value::Node> {
    let mutation_params = path_node_ctx
        .rescaling_ctx
        .current_rescaling
        .rescale_mutation(mutation_params);

    let initial_value;

    let value_to_mutate = if is_optional {
        let flip = Bernoulli::new(mutation_params.mutation_prob)
            .unwrap()
            .sample(rng);

        match (flip, value) {
            (true, Some(_)) => None,
            (false, value @ Some(_)) => value,
            (true, None) => {
                initial_value = Some(spec_node.initial_value());
                initial_value.as_ref()
            }
            _ => unreachable!(),
        }
    } else {
        value
    };

    value_to_mutate.map(|value| {
        do_mutate_value_present(value, spec_node, &mutation_params, path_node_ctx, rng)
    })
}

fn do_mutate_value_present(
    value: &value::Node,
    spec_node: &spec::Node,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    match (spec_node, value) {
        (
            spec::Node::Real {
                scale, min, max, ..
            },
            value::Node::Real(value),
        ) => value::Node::Real(mutate_real(
            *value,
            *scale,
            *min,
            *max,
            mutation_params,
            rng,
        )),
        (
            spec::Node::Int {
                scale, min, max, ..
            },
            value::Node::Int(value),
        ) => value::Node::Int(mutate_int(*value, *scale, *min, *max, mutation_params, rng)),
        (spec::Node::Bool { .. }, value::Node::Bool(value)) => {
            value::Node::Bool(mutate_bool(*value, mutation_params, rng))
        }

        (spec::Node::Sub { map, .. }, value::Node::Sub(value_map)) => {
            mutate_sub(map, value_map, mutation_params, path_node_ctx, rng)
        }
        (
            spec::Node::AnonMap {
                value_type,
                min_size,
                max_size,
                ..
            },
            value::Node::AnonMap(value_map),
        ) => mutate_anon_map(
            value_type,
            min_size,
            max_size,
            value_map,
            mutation_params,
            path_node_ctx,
            rng,
        ),
        _ => unreachable!(),
    }
}

fn mutate_sub(
    spec_map: &HashMap<String, Box<spec::Node>>,
    value_map: &HashMap<String, Box<value::Node>>,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    let result_mapping = spec_map
        .iter()
        .map(|(key, child_spec)| {
            let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(key);
            let child_value_node = value_map.get(key).map(Box::as_ref);

            let mutated_child_value_node = do_mutate(
                child_value_node,
                child_spec,
                spec_util::is_optional(child_spec),
                mutation_params,
                child_path_node_ctx,
                rng,
            );

            (key, mutated_child_value_node)
        })
        .filter_map(|(child_key, child_val)| {
            child_val.map(|present_value| (child_key.clone(), Box::new(present_value)))
        })
        .collect();

    value::Node::Sub(result_mapping)
}

lazy_static! {
    static ref COIN_FLIP: Bernoulli = Bernoulli::new(0.5).unwrap();
}

fn mutate_anon_map(
    value_type: &spec::Node,
    min_size: &Option<usize>,
    max_size: &Option<usize>,
    value_map: &HashMap<usize, Box<value::Node>>,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    let resize = Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng);

    let mut value_map = value_map.clone();
    if resize {
        let is_at_min_size = value_map.is_empty()
            || min_size
                .map(|size| value_map.len() == size)
                .unwrap_or(false);
        let is_at_max_size = max_size
            .map(|size| value_map.len() == size)
            .unwrap_or(false);

        let remove_one = !is_at_min_size && (is_at_max_size || COIN_FLIP.sample(rng));

        if remove_one {
            let key_to_remove = *value_map.keys().choose(rng).unwrap();
            value_map.remove(&key_to_remove);
        } else {
            let key = path_node_ctx.next_key();
            value_map.insert(key, Box::new(value_type.initial_value()));
        }
    };

    value::Node::AnonMap(
        value_map
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    Box::new(
                        do_mutate(
                            Some(&value),
                            value_type,
                            false,
                            mutation_params,
                            path_node_ctx,
                            rng,
                        )
                        .unwrap(),
                    ),
                )
            })
            .collect(),
    )
}

fn mutate_real(
    value: f64,
    scale: f64,
    min: Option<f64>,
    max: Option<f64>,
    mutation_params: &MutationParams,
    rng: &mut StdRng,
) -> f64 {
    if Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng)
    {
        let mut value = Cauchy::new(value, scale * mutation_params.mutation_scale)
            .unwrap()
            .sample(rng);

        if let Some(min) = min {
            value = value.max(min);
        }

        if let Some(max) = max {
            value = value.min(max);
        }

        value
    } else {
        value
    }
}

fn mutate_int(
    value: i64,
    scale: f64,
    min: Option<i64>,
    max: Option<i64>,
    mutation_params: &MutationParams,
    rng: &mut StdRng,
) -> i64 {
    if Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng)
    {
        let mut value = Cauchy::new(
            value.to_f64().unwrap(),
            scale * mutation_params.mutation_scale,
        )
        .unwrap()
        .sample(rng)
        .to_i64()
        .unwrap();

        if let Some(min) = min {
            value = value.max(min);
        }

        if let Some(max) = max {
            value = value.min(max);
        }

        value
    } else {
        value
    }
}

fn mutate_bool(value: bool, mutation_params: &MutationParams, rng: &mut StdRng) -> bool {
    value
        ^ Bernoulli::new(mutation_params.mutation_prob)
            .unwrap()
            .sample(rng)
}

#[cfg(test)]
mod tests {
    use crate::path::testutil::set_rescaling_at_path;
    use crate::rescaling::CrossoverRescaling;
    use crate::rescaling::MutationRescaling;
    use crate::rescaling::Rescaling;
    use crate::testutil::extract_as_anon_map;
    use crate::testutil::extract_as_bool;
    use crate::testutil::extract_as_int;
    use std::collections::HashSet;

    use finite::FiniteF64;
    use float_cmp::approx_eq;
    use lazy_static::__Deref;
    use rand::SeedableRng;

    use crate::{testutil::extract_as_real, value_util};

    use super::*;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(0)
    }

    fn make_rescaling(mutation_prob_factor: f64, mutation_scale_factor: f64) -> Rescaling {
        Rescaling {
            crossover_rescaling: CrossoverRescaling::default(),
            mutation_rescaling: MutationRescaling {
                mutation_prob_factor,
                mutation_scale_factor,
            },
        }
    }

    fn never_mutate_rescaling() -> Rescaling {
        make_rescaling(0.0, 1.0)
    }

    #[test]
    fn mutate_bool_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_bool(true, &mutation_params, &mut rng());

        assert!(result);
    }

    #[test]
    fn mutate_bool_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1.0,
        };

        let result = mutate_bool(true, &mutation_params, &mut rng());

        assert!(!result);
    }

    #[test]
    fn mutate_int_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_int(10, 10.0, None, None, &mutation_params, &mut rng());

        assert_eq!(result, 10);
    }

    #[test]
    fn mutate_int_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        const N: usize = 100;

        let mut found_changed = false;
        for _ in 0..N {
            let result = mutate_int(10, 10.0, None, None, &mutation_params, &mut rng);

            if result != 10 {
                found_changed = true;
            }
        }

        assert!(found_changed);
    }

    #[test]
    fn mutate_int_near_zero_scale() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1e-9,
        };

        let mut rng = rng();
        const N: usize = 100;

        let mut found_changed = false;
        for _ in 0..N {
            let result = mutate_int(10, 10.0, None, None, &mutation_params, &mut rng);

            if result != 10 && result != 9 {
                found_changed = true;
            }
        }

        assert!(!found_changed);
    }

    #[test]
    fn mutate_int_min_and_max() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        const N: usize = 500;

        let mut values = HashSet::new();

        for _ in 0..N {
            let result = mutate_int(10, 10.0, Some(9), Some(11), &mutation_params, &mut rng);
            values.insert(result);
        }

        assert_eq!(values, HashSet::from([9, 10, 11]));
    }

    #[test]
    fn mutate_real_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_real(10.0, 10.0, None, None, &mutation_params, &mut rng());

        assert_eq!(result, 10.0);
    }

    #[test]
    fn mutate_real_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1.0,
        };

        let result = mutate_real(10.0, 10.0, None, None, &mutation_params, &mut rng());

        assert_ne!(result, 10.0);
    }

    #[test]
    fn mutate_real_min_and_max() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        const N: usize = 500;

        let mut values = Vec::new();

        for _ in 0..N {
            let result = mutate_real(
                10.0,
                10.0,
                Some(9.0),
                Some(11.0),
                &mutation_params,
                &mut rng,
            );
            values.push(result);
        }

        let min_found = values
            .iter()
            .map(|val| FiniteF64::new(*val).unwrap())
            .min()
            .unwrap();

        assert!(min_found >= 9.0);
        assert!(min_found < 10.0);

        let max_found = values
            .iter()
            .map(|val| FiniteF64::new(*val).unwrap())
            .max()
            .unwrap();

        assert!(max_found <= 11.0);
        assert!(max_found > 10.0);
    }

    #[test]
    fn mutate_sub() {
        let spec_str = "
        real_a:
            type: real
            init: 0
            scale: 1
            optional: true
        real_b:
            type: real
            init: 0
            scale: 1
            optional: true
        real_c:
            type: real
            init: 0
            scale: 1
        int_a:
            type: int
            init: 0
            scale: 1
            optional: true
        int_b:
            type: int
            init: 0
            scale: 1
            optional: true
        int_c:
            type: int
            init: 0
            scale: 1
        bool_a:
            type: bool
            init: false
        ";

        let value_str = r#"
        {
            "real_a": 1,
            "real_c": 2,
            "int_a": 3,
            "int_c": 4,
            "bool_a": true
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert!(extract_as_real(&result, &["real_a"]).is_none());

        // note: bitwise float cmp is intentional. The test will legitimately fail if real values aren't mutated
        assert!(matches!(extract_as_real(&result, &["real_b"]), Some(value) if value != 0.0));
        assert!(matches!(extract_as_real(&result, &["real_c"]), Some(value) if value != 2.0));

        assert!(extract_as_int(&result, &["int_a"]).is_none());
        assert!(extract_as_int(&result, &["int_b"]).is_some());
        assert!(extract_as_int(&result, &["int_c"]).is_some());

        assert!(!extract_as_bool(&result, &["bool_a"]).unwrap());
    }

    #[test]
    fn mutate_anon_map() {
        let spec_str = "
        type: anon map
        valueType:
            type: bool
            init: false
        initSize: 1
        minSize: 1
        maxSize: 4
        ";

        let value_str = r#"
        {
            "0": false,
            "1": false
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let mut value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let mut out_values = Vec::new();

        const N: usize = 1000;
        for _ in 0..N {
            let mutated_value = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

            let original_map = extract_as_anon_map(&value, &[]).unwrap();
            let mutated_map = extract_as_anon_map(&mutated_value, &[]).unwrap();

            if original_map.len() > 1 && original_map.len() < 4 {
                assert!(mutated_map.len() != original_map.len());
            }

            for (key, mutated_val) in mutated_map {
                if let value::Node::Bool(mutated_val) = *mutated_val {
                    let original_val = original_map.get(&key);
                    match original_val.map(Box::deref) {
                        Some(value::Node::Bool(original_val)) => {
                            assert_ne!(mutated_val, *original_val);
                        }
                        None => {
                            assert!(mutated_val);
                        }
                        _ => unreachable!(),
                    }
                } else {
                    unreachable!()
                }
            }

            value = mutated_value;
            out_values.push(value.clone());
        }

        let inner_maps: Vec<HashMap<usize, Box<value::Node>>> = out_values
            .into_iter()
            .map(|val| extract_as_anon_map(&val, &[]).unwrap())
            .collect();

        let min_size = inner_maps.iter().map(HashMap::len).min().unwrap();
        let max_size = inner_maps.iter().map(HashMap::len).max().unwrap();

        assert_eq!(min_size, 1);
        assert_eq!(max_size, 4);
    }

    #[test]
    fn rescale_mut_prob() {
        let spec_str = "
        foo:
            type: bool
            init: false
        ";

        let value_str = r#"
        {
            "foo": false
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let rescaling = never_mutate_rescaling();

        set_rescaling_at_path(&mut path_ctx.0, &["foo"], rescaling);
        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert!(!extract_as_bool(&result, &["foo"]).unwrap());
    }

    #[test]
    fn rescale_mut_prob_anon_map_deep() {
        let spec_str = "
        type: anon map
        valueType: 
            foo:
                type: bool
                init: false
        minSize: 1
        ";

        let value_str = r#"
        {
            "0": {
                "foo": false
            }
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let rescaling = never_mutate_rescaling();

        set_rescaling_at_path(&mut path_ctx.0, &["0", "foo"], rescaling);
        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(extract_as_anon_map(&result, &[]).unwrap().len(), 2);
        assert!(extract_as_bool(&result, &["0", "foo"]).unwrap());
        assert!(extract_as_bool(&result, &["1", "foo"]).unwrap());
    }

    #[test]
    fn rescale_mut_scale() {
        let spec_str = "
        type: real
        init: 0
        scale: 1
        ";

        let value_str = r#"
        1.0
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let mutation_scale_factor = 1e-9;
        let mutation_prob_factor = 1.0;
        let rescaling = make_rescaling(mutation_prob_factor, mutation_scale_factor);

        set_rescaling_at_path(&mut path_ctx.0, &[], rescaling);
        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);
        let result = extract_as_real(&result, &[]).unwrap();

        assert!(approx_eq!(f64, result, 1.0, epsilon = 1e-6));
    }

    #[test]
    fn stochstic_scenario() {
        let spec_str = "
        foo:
            bar:
                type: bool
                init: false
            optional: true
        ";

        let value_str = r#"
        {
        }
        "#;

        let other_value_str = r#"
        {
            "foo": {
                "bar": false
            }
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();
        let val_for_path_prep = value_util::from_json_str(other_value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&val_for_path_prep.0);

        let mutation_prob_factor = 0.5;
        let mutation_scale_factor = 1.0;
        let rescaling = make_rescaling(mutation_prob_factor, mutation_scale_factor);

        set_rescaling_at_path(&mut path_ctx.0, &["foo", "bar"], rescaling);

        const N: usize = 10000;

        let mut true_count = 0;

        for _ in 0..N {
            let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);
            let result = extract_as_bool(&result, &["foo", "bar"]).unwrap_or(false);
            if result {
                true_count += 1;
            }
        }

        let true_fraction = true_count.to_f64().unwrap() / N.to_f64().unwrap();
        assert!(approx_eq!(f64, true_fraction, 0.5, epsilon = 5e-2));
    }
}
