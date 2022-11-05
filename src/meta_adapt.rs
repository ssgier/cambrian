use crate::meta::{CrossoverParams, MutationParams};
use rand::distributions::Uniform;
use rand::rngs::StdRng;
use rand_distr::{Cauchy, Distribution};

const META_PARAMS_MUTATION_EXPONENT_SCALE: f64 = 1.0;
const META_PARAMS_MUTATION_RESCALE_FLOOR: f64 = 1e-12;
const META_PARAMS_MUTATION_RESCALE_CEIL: f64 = 1e12;

fn sample_exponential(rng: &mut StdRng) -> f64 {
    let uniform_excl_zero: f64 = 1.0 - Uniform::new(0.0, 1.0).sample(rng);
    -uniform_excl_zero.ln()
}

fn sample_prob(rng: &mut StdRng) -> f64 {
    Uniform::new_inclusive(0.0, 1.0).sample(rng)
}

fn rescale(value: f64, rng: &mut StdRng) -> f64 {
    let exponent: f64 = Cauchy::new(1.0, META_PARAMS_MUTATION_EXPONENT_SCALE)
        .unwrap()
        .sample(rng);

    value
        .powf(exponent)
        .max(META_PARAMS_MUTATION_RESCALE_FLOOR)
        .min(META_PARAMS_MUTATION_RESCALE_CEIL)
}

fn rescale_prob(prob: f64, rng: &mut StdRng) -> f64 {
    rescale(prob, rng).min(1.0)
}

pub fn create_exploratory(rng: &mut StdRng) -> (CrossoverParams, MutationParams) {
    let crossover_params = CrossoverParams {
        crossover_prob: sample_prob(rng),
        selection_pressure: sample_prob(rng),
    };

    let mutation_params = MutationParams {
        mutation_prob: sample_prob(rng),
        mutation_scale: sample_exponential(rng),
    };

    (crossover_params, mutation_params)
}

pub fn mutate(
    crossover_params: CrossoverParams,
    mutation_params: MutationParams,
    rng: &mut StdRng,
) -> (CrossoverParams, MutationParams) {
    let crossover_params = CrossoverParams {
        crossover_prob: rescale_prob(crossover_params.crossover_prob, rng),
        selection_pressure: rescale_prob(crossover_params.selection_pressure, rng),
    };

    let mutation_params = MutationParams {
        mutation_prob: rescale_prob(mutation_params.mutation_prob, rng),
        mutation_scale: rescale(mutation_params.mutation_scale, rng),
    };

    (crossover_params, mutation_params)
}
