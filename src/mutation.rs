use crate::meta::MutationParams;
use crate::path::{PathContext, PathNodeContext};
use crate::spec;
use crate::value;
use crate::value::Value;
use lazy_static::{__Deref, lazy_static};
use rand::rngs::StdRng;
use rand::seq::IteratorRandom;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Bernoulli, Cauchy, Distribution};
use crate::types::HashMap;

pub fn mutate(
    spec: &spec::Spec,
    individual: &Value,
    mutation_params: &MutationParams,
    path_ctx: &mut PathContext,
    rng: &mut StdRng,
) -> Value {
    let spec_node = &spec.0;
    let path_node_ctx = &mut path_ctx.0;

    let rescaled_mutation_params = path_node_ctx
        .rescaling_ctx
        .current_rescaling
        .rescale_mutation(mutation_params);

    Value(do_mutate(
        &individual.0,
        spec_node,
        &rescaled_mutation_params,
        path_node_ctx,
        rng,
    ))
}

fn do_mutate(
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
        (
            spec::Node::Variant { map: spec_map, .. },
            value::Node::Variant(current_variant_name, current_value),
        ) => mutate_variant(
            spec_map,
            current_variant_name,
            current_value,
            mutation_params,
            path_node_ctx,
            rng,
        ),
        (spec::Node::Enum { values, .. }, value::Node::Enum(current_name)) => {
            mutate_enum(values, current_name, mutation_params, path_node_ctx, rng)
        }
        (spec::Node::Optional { value_type, .. }, value::Node::Optional(value_option)) => {
            mutate_optional(
                value_option.as_ref().map(|x| x.deref()),
                value_type,
                mutation_params,
                path_node_ctx,
                rng,
            )
        }
        (spec::Node::Const, _) => value::Node::Const,
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
            let child_value_node = value_map.get(key).map(Box::as_ref).unwrap();

            let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(key);
            let child_mutation_params = child_path_node_ctx
                .rescaling_ctx
                .current_rescaling
                .rescale_mutation(mutation_params);

            let mutated_child_value_node = do_mutate(
                child_value_node,
                child_spec,
                &child_mutation_params,
                child_path_node_ctx,
                rng,
            );

            (key, mutated_child_value_node)
        })
        .map(|(child_key, child_val)| (child_key.clone(), Box::new(child_val)))
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

    let mut key_to_remove: Option<usize> = None;
    let mut key_value_pair_to_add: Option<(usize, value::Node)> = None;

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
            key_to_remove = Some(*value_map.keys().choose(rng).unwrap());
        } else {
            let value = value_map
                .values()
                .choose(rng)
                .map(Box::deref)
                .cloned()
                .unwrap_or_else(|| value_type.initial_value());

            let key = path_node_ctx.next_key();
            let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(&key.to_string());
            let child_mutation_params = child_path_node_ctx
                .rescaling_ctx
                .current_rescaling
                .rescale_mutation(mutation_params);

            let mutated_value_to_add = do_mutate(
                &value,
                value_type,
                &child_mutation_params,
                child_path_node_ctx,
                rng,
            );

            key_value_pair_to_add = Some((key, mutated_value_to_add));
        }
    };

    let mut result_map: HashMap<usize, Box<value::Node>> = value_map
        .iter()
        .map(|(key, value)| {
            let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(&key.to_string());
            let child_mutation_params = child_path_node_ctx
                .rescaling_ctx
                .current_rescaling
                .rescale_mutation(mutation_params);

            (
                *key,
                Box::new(do_mutate(
                    value,
                    value_type,
                    &child_mutation_params,
                    child_path_node_ctx,
                    rng,
                )),
            )
        })
        .collect();

    match (key_to_remove, key_value_pair_to_add) {
        (Some(key_to_remove), None) => {
            result_map.remove(&key_to_remove);
        }
        (None, Some((key, value))) => {
            result_map.insert(key, Box::new(value));
        }
        (None, None) => (),
        (Some(_), Some(_)) => unreachable!(),
    }

    value::Node::AnonMap(result_map)
}

fn mutate_variant(
    spec_map: &HashMap<String, Box<spec::Node>>,
    current_variant_name: &str,
    value: &value::Node,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    let change_variant = Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng);

    let new_init_value;
    let (out_variant_name, pre_mutation_val) = if change_variant {
        let new_variant_name = spec_map
            .keys()
            .filter(|key| *key != current_variant_name)
            .choose(rng)
            .unwrap();
        new_init_value = spec_map.get(new_variant_name).unwrap().initial_value();

        (new_variant_name.as_str(), &new_init_value)
    } else {
        (current_variant_name, value)
    };

    let child_path_node_ctx = path_node_ctx.get_or_create_child_mut(out_variant_name);
    let child_mutation_params = child_path_node_ctx
        .rescaling_ctx
        .current_rescaling
        .rescale_mutation(mutation_params);

    let mutated_child_value_node = do_mutate(
        pre_mutation_val,
        spec_map.get(out_variant_name).unwrap(),
        &child_mutation_params,
        child_path_node_ctx,
        rng,
    );

    value::Node::Variant(
        out_variant_name.to_owned(),
        Box::new(mutated_child_value_node),
    )
}

fn mutate_enum(
    spec_values: &[String],
    current_value: &str,
    mutation_params: &MutationParams,
    _path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    let change_variant = Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng);

    let new_value = if change_variant {
        spec_values
            .iter()
            .filter(|name| *name != current_value)
            .choose(rng)
            .unwrap()
    } else {
        current_value
    };

    value::Node::Enum(new_value.to_owned())
}

fn mutate_optional(
    value: Option<&value::Node>,
    spec_node: &spec::Node,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    let initial_value;

    let flip = Bernoulli::new(mutation_params.mutation_prob)
        .unwrap()
        .sample(rng);

    let value_option_to_mutate = match (flip, value) {
        (true, Some(_)) => None,
        (true, None) => {
            initial_value = Some(spec_node.initial_value());
            initial_value.as_ref()
        }
        (false, value) => value
    };

    value::Node::Optional(value_option_to_mutate.map(|value| {
        let child_path_node_ctx = path_node_ctx.get_or_create_child_mut("optional");
        let child_mutation_params = child_path_node_ctx
            .rescaling_ctx
            .current_rescaling
            .rescale_mutation(mutation_params);

        Box::new(do_mutate(
            value,
            spec_node,
            &child_mutation_params,
            child_path_node_ctx,
            rng,
        ))
    }))
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
        .round()
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
    use crate::spec_util;
    use crate::testutil::extract_as_anon_map;
    use crate::testutil::extract_as_bool;
    use crate::testutil::extract_as_int;
    use crate::{testutil::extract_as_real, value_util};
    use float_cmp::approx_eq;
    use lazy_static::__Deref;
    use rand::SeedableRng;
    use crate::types::HashSet;
    use tangram_finite::FiniteF64;

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
    fn mutate_const() {
        let spec_str = "
        type: const
        ";

        let value_str = "null";

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

        assert_eq!(result.0, value::Node::Const);
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
    fn mutate_enum_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1.0,
        };

        let current_value = "foo";
        let spec_values = vec!["foo".to_string(), "bar".to_string()];

        let mut path_node_ctx = PathContext::default().0;

        let result = mutate_enum(
            &spec_values,
            current_value,
            &mutation_params,
            &mut path_node_ctx,
            &mut rng(),
        );

        assert!(result == value::Node::Enum("bar".to_string()));
    }

    #[test]
    fn mutate_enum_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 1.0,
        };

        let current_value = "foo";
        let spec_values = vec!["foo".to_string(), "bar".to_string()];

        let mut path_node_ctx = PathContext::default().0;

        let result = mutate_enum(
            &spec_values,
            current_value,
            &mutation_params,
            &mut path_node_ctx,
            &mut rng(),
        );

        assert!(result == value::Node::Enum("foo".to_string()));
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

        let mut values = HashSet::default();

        for _ in 0..N {
            let result = mutate_int(10, 10.0, Some(9), Some(11), &mutation_params, &mut rng);
            values.insert(result);
        }

        assert_eq!(values, HashSet::from_iter([9, 10, 11]));
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

        assert!(min_found.get() >= 9.0);
        assert!(min_found.get() < 10.0);

        let max_found = values
            .iter()
            .map(|val| FiniteF64::new(*val).unwrap())
            .max()
            .unwrap();

        assert!(max_found.get() <= 11.0);
        assert!(max_found.get() > 10.0);
    }

    #[test]
    fn mutate_optional_some_to_none() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: false
        ";

        let value_str = "true";

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

        assert_eq!(result.0, value::Node::Optional(None));
    }

    #[test]
    fn mutate_optional_none_to_some() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: false
        ";

        let value_str = "null";

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

        assert_eq!(
            result.0,
            value::Node::Optional(Some(Box::new(value::Node::Bool(true))))
        );
    }

    #[test]
    fn mutate_optional_guaranteed_not() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: false
        ";

        let value_str = "false";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(
            result.0,
            value::Node::Optional(Some(Box::new(value::Node::Bool(false))))
        );
    }

    #[test]
    fn mutate_optional_one_deep() {
        let spec_str = "
        type: optional
        initPresent: false
        valueType:
            type: bool
            init: false
        ";

        let value_str = "null";
        let other_value_str = "false";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();
        let other_value = value_util::from_json_str(other_value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);
        path_ctx.0.add_nodes_for(&other_value.0);
        let rescaling = never_mutate_rescaling();
        set_rescaling_at_path(&mut path_ctx.0, &["optional"], rescaling);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(
            result.0,
            value::Node::Optional(Some(Box::new(value::Node::Bool(false))))
        );
    }

    #[test]
    fn mutate_variant_guaranteed() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: false
        ";

        let value_str = "{
            \"foo\": 1
        }";

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

        assert_eq!(
            result.0,
            value::Node::Variant("bar".to_string(), Box::new(value::Node::Bool(true)))
        );
    }

    #[test]
    fn mutate_variant_guaranteed_not() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: false
        ";

        let value_str = "{
            \"foo\": 1
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(
            result.0,
            value::Node::Variant("foo".to_string(), Box::new(value::Node::Int(1)))
        );
    }

    #[test]
    fn mutate_variant_one_deep() {
        let spec_str = "
        type: variant
        init: foo
        foo:
            type: int
            init: 0
            scale: 1
        bar:
            type: bool
            init: false
        ";

        let value_str = "{
            \"foo\": 1
        }";

        let other_value_str = "{
            \"bar\": false
        }";

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();
        let other_value = value_util::from_json_str(other_value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 10.0,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);
        path_ctx.0.add_nodes_for(&other_value.0);
        let rescaling = never_mutate_rescaling();
        set_rescaling_at_path(&mut path_ctx.0, &["bar"], rescaling);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(
            result.0,
            value::Node::Variant("bar".to_string(), Box::new(value::Node::Bool(false)))
        );
    }

    #[test]
    fn mutate_sub() {
        let spec_str = "
        real_a:
            type: optional
            initPresent: false
            valueType:
                type: real
                init: 0
                scale: 1
        real_b:
            type: optional
            initPresent: false
            valueType:
                type: real
                init: 0
                scale: 1
        real_c:
            type: real
            init: 0
            scale: 1
        int_a:
            type: optional
            initPresent: false
            valueType:
                type: int
                init: 0
                scale: 1
        int_b:
            type: optional
            initPresent: false
            valueType:
                type: int
                init: 0
                scale: 1
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
            "real_b": null,
            "real_c": 2,
            "int_a": 3,
            "int_b": null,
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

        assert!(extract_as_real(&result, &["real_a", "optional"]).is_none());

        // note: bitwise float cmp is intentional. The test will legitimately fail if real values aren't mutated
        assert!(
            matches!(extract_as_real(&result, &["real_b", "optional"]), Some(value) if value != 0.0)
        );
        assert!(matches!(extract_as_real(&result, &["real_c"]), Some(value) if value != 2.0));

        assert!(extract_as_int(&result, &["int_a", "optional"]).is_none());
        assert!(extract_as_int(&result, &["int_b", "optional"]).is_some());
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
                    if let Some(value::Node::Bool(original_val)) = original_val.map(Box::deref) {
                        assert_ne!(mutated_val, *original_val);
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
    fn mutate_non_empty_anon_map() {
        let spec_str = "
        type: anon map
        initSize: 1
        minSize: 1
        valueType:
            type: int
            init: 1
            scale: 1
        ";

        let value_str = r#"
        {
            "0": 10
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1e-12,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(extract_as_int(&result, &["1"]).unwrap(), 10);
    }

    #[test]
    fn mutate_empty_anon_map() {
        let spec_str = "
        type: anon map
        initSize: 1
        minSize: 0
        valueType:
            type: int
            init: 1
            scale: 1
        ";

        let value_str = r#"
        {
        }
        "#;

        let spec = spec_util::from_yaml_str(spec_str).unwrap();
        let value = value_util::from_json_str(value_str, &spec).unwrap();

        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            mutation_scale: 1e-12,
        };

        let mut rng = rng();
        let mut path_ctx = PathContext::default();
        path_ctx.0.add_nodes_for(&value.0);

        let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);

        assert_eq!(extract_as_int(&result, &["0"]).unwrap(), 1);
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
        initSize: 1
        minSize: 1
        valueType: 
            foo:
                type: bool
                init: false
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
        assert!(!extract_as_bool(&result, &["0", "foo"]).unwrap());
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
            type: optional
            initPresent: false
            valueType:
                bar:
                    type: bool
                    init: false
        ";

        let value_str = r#"
        {
            "foo": null
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

        set_rescaling_at_path(&mut path_ctx.0, &["foo", "optional", "bar"], rescaling);

        const N: usize = 10000;

        let mut true_count = 0;

        for _ in 0..N {
            let result = mutate(&spec, &value, &mutation_params, &mut path_ctx, &mut rng);
            let result = extract_as_bool(&result, &["foo", "optional", "bar"]).unwrap_or(false);
            if result {
                true_count += 1;
            }
        }

        let true_fraction = true_count.to_f64().unwrap() / N.to_f64().unwrap();
        assert!(approx_eq!(f64, true_fraction, 0.5, epsilon = 5e-2));
    }
}
