use crate::meta::MutationParams;
use crate::path::{Path, PathNode};
use rand::rngs::StdRng;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Bernoulli, Cauchy, Distribution};

use crate::spec;
use crate::value::Node::{self, *};
use crate::{spec::Spec, value::Value};

pub struct Mutation {}

impl Mutation {
    pub fn mutate(
        &self,
        spec: &spec::Spec,
        individual: &Value,
        mutation_params: &MutationParams,
        path: &mut Path,
        rng: &mut StdRng,
    ) -> Value {
        Value(self.do_mutate(&individual.0, &spec.0, mutation_params, &mut path.0, rng))
    }

    fn do_mutate(
        &self,
        _individual: &Node,
        _spec: &spec::Node,
        mutation_params: &MutationParams,
        path_node: &mut PathNode,
        _rng: &mut StdRng,
    ) -> Node {
        let mutation_params = path_node
            .rescaling_ctx
            .current_rescaling
            .rescale_mutation(mutation_params);

        panic!("not implemented")
    }

    fn mutate_real(
        value: f64,
        scale: f64,
        min: Option<f64>,
        max: Option<f64>,
        mutation_params: &MutationParams,
        rng: &mut StdRng,
    ) -> Node {
        let result_val = if Bernoulli::new(mutation_params.mutation_prob)
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
        };
        Real(result_val)
    }

    fn mutate_int(
        value: i64,
        scale: f64,
        min: Option<i64>,
        max: Option<i64>,
        mutation_params: &MutationParams,
        rng: &mut StdRng,
    ) -> Node {
        let result_val = if Bernoulli::new(mutation_params.mutation_prob)
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
        };

        Int(result_val)
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
    use rand::SeedableRng;

    use super::*;

    fn rng() -> StdRng {
        StdRng::seed_from_u64(0)
    }

    #[test]
    fn mutate_bool_guaranteed_not() {
        let mutation_params = MutationParams {
            mutation_prob: 0.0,
            mutation_scale: 1.0, // TODO, this shouldn't be here. Rethink design
        };

        // TODO
    }
}
