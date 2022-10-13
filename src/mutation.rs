use crate::meta::MutationParams;
use crate::path::{PathContext, PathNodeContext};
use rand::rngs::StdRng;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Bernoulli, Cauchy, Distribution};
use std::collections::HashMap;

use crate::value;
use crate::value::Value;
use crate::{spec, spec_util};

pub struct Mutation {}

impl Mutation {
    pub fn mutate(
        &self,
        spec: &spec::Spec,
        individual: &Value,
        mutation_params: &MutationParams,
        path_ctx: &mut PathContext,
        rng: &mut StdRng,
    ) -> Value {
        Value(
            self.do_mutate(
                Some(&individual.0),
                &spec.0,
                mutation_params,
                &mut path_ctx.0,
                rng,
            )
            .unwrap(),
        )
    }

    fn do_mutate(
        &self,
        value: Option<&value::Node>,
        spec_node: &spec::Node,
        mutation_params: &MutationParams,
        path_node_ctx: &mut PathNodeContext,
        rng: &mut StdRng,
    ) -> Option<value::Node> {
        let mutation_params = path_node_ctx
            .rescaling_ctx
            .current_rescaling
            .rescale_mutation(mutation_params);

        let is_value_optional = match spec_node {
            spec::Node::Real { optional, .. }
            | spec::Node::Int { optional, .. }
            | spec::Node::Sub { optional, .. }
            | spec::Node::AnonMap { optional, .. } => *optional,
            spec::Node::Bool { .. } => false,
        };

        let initial_value;

        let value_to_mutate = if is_value_optional {
            let flip = Bernoulli::new(mutation_params.flip_prob)
                .unwrap()
                .sample(rng);

            match (flip, value) {
                (true, Some(_)) | (false, None) => None,
                (false, value @ Some(_)) => value,
                (true, None) => {
                    initial_value = Some(spec_node.initial_value());
                    initial_value.as_ref()
                }
            }
        } else {
            value
        };

        value_to_mutate.map(|value| {
            self.do_mutate_value_present(value, spec_node, &mutation_params, path_node_ctx, rng)
        })
    }

    fn do_mutate_value_present(
        &self,
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
            ) => mutate_anon_map(value_type, value_map, mutation_params, path_node_ctx, rng),
            _ => panic!("Spec violation"),
        }
    }
}

fn mutate_sub(
    spec_map: &HashMap<String, Box<spec::Node>>,
    value_map: &HashMap<String, Box<value::Node>>,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    panic!("not implemented");
}

fn mutate_anon_map(
    value_type: &spec::Node,
    value_map: &HashMap<usize, Box<value::Node>>,
    mutation_params: &MutationParams,
    path_node_ctx: &mut PathNodeContext,
    rng: &mut StdRng,
) -> value::Node {
    panic!("not implemented");
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
    use std::collections::HashSet;

    use finite::FiniteF64;
    use rand::SeedableRng;

    use super::*;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(0)
    }

    #[test]
    fn mutate_bool_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            flip_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_bool(true, &mutation_params, &mut rng());

        assert!(result);
    }

    #[test]
    fn mutate_bool_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            flip_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_bool(true, &mutation_params, &mut rng());

        assert!(!result);
    }

    #[test]
    fn mutate_int_guaranteed_not() {
        let mutation_params = MutationParams {
            flip_prob: 0.0,
            mutation_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_int(10, 10.0, None, None, &mutation_params, &mut rng());

        assert_eq!(result, 10);
    }

    #[test]
    fn mutate_int_guaranteed() {
        let mutation_params = MutationParams {
            flip_prob: 0.0,
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
            flip_prob: 0.0,
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
            flip_prob: 0.0,
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
            flip_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_real(10.0, 10.0, None, None, &mutation_params, &mut rng());

        assert_eq!(result, 10.0);
    }

    #[test]
    fn mutate_real_guaranteed() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            flip_prob: 0.0,
            mutation_scale: 1.0,
        };

        let result = mutate_real(10.0, 10.0, None, None, &mutation_params, &mut rng());

        assert_ne!(result, 10.0);
    }

    #[test]
    fn mutate_real_min_and_max() {
        let mutation_params = MutationParams {
            mutation_prob: 1.0,
            flip_prob: 0.0,
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
}
