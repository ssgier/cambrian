use crate::meta::{CrossoverParams, MutationParams};
use rand::rngs::StdRng;
use rand_distr::{Cauchy, Distribution};

const META_PARAMS_MUTATION_EXPONENT_SCALE: f64 = 0.1;
const META_PARAMS_MUTATION_RESCALE_FLOOR: f64 = 1e-12;
const META_PARAMS_MUTATION_RESCALE_CEIL: f64 = 1e12;

fn rescale(value: f64, rng: &mut StdRng) -> f64 {
    let exponent: f64 = Cauchy::new(0.0, META_PARAMS_MUTATION_EXPONENT_SCALE)
        .unwrap()
        .sample(rng);

    let factor = 10.0f64.powf(exponent);

    value
        * factor
            .max(META_PARAMS_MUTATION_RESCALE_FLOOR)
            .min(META_PARAMS_MUTATION_RESCALE_CEIL)
}

fn rescale_prob(prob: f64, rng: &mut StdRng) -> f64 {
    rescale(prob, rng).min(1.0)
}

pub fn create_exploratory(rng: &mut StdRng) -> (CrossoverParams, MutationParams) {
    let crossover_params = CrossoverParams {
        crossover_prob: 0.5,
        selection_pressure: 0.5,
    };

    let mutation_params = MutationParams {
        mutation_prob: 0.5,
        mutation_scale: 1.0,
    };

    mutate(crossover_params, mutation_params, rng)
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

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::assert_approx_eq;
    use rand::SeedableRng;

    fn assert_min_less_than(vals: &[f64], cmp: f64) {
        assert!(*vals.iter().min_by(|a, b| a.total_cmp(b)).unwrap() < cmp);
    }

    fn assert_max_greater_than(vals: &[f64], cmp: f64) {
        assert!(*vals.iter().max_by(|a, b| a.total_cmp(b)).unwrap() > cmp);
    }

    fn assert_max_equal_to(vals: &[f64], cmp: f64) {
        assert_approx_eq!(
            f64,
            *vals.iter().max_by(|a, b| a.total_cmp(b)).unwrap(),
            cmp
        );
    }

    #[test]
    fn mutate_covers_orders_of_magnitude() {
        let mut rng = StdRng::seed_from_u64(0);

        let crossover_params = CrossoverParams {
            crossover_prob: 0.5,
            selection_pressure: 0.5,
        };

        let mutation_params = MutationParams {
            mutation_prob: 0.5,
            mutation_scale: 0.5,
        };

        let mut crossover_probs = Vec::new();
        let mut selection_pressures = Vec::new();
        let mut mutation_probs = Vec::new();
        let mut mutation_scales = Vec::new();

        for _ in 0..100 {
            let (crossover_params, mutation_params) =
                mutate(crossover_params.clone(), mutation_params.clone(), &mut rng);

            crossover_probs.push(crossover_params.crossover_prob);
            selection_pressures.push(crossover_params.selection_pressure);
            mutation_probs.push(mutation_params.mutation_prob);
            mutation_scales.push(mutation_params.mutation_scale);
        }

        for vals in [crossover_probs, selection_pressures, mutation_probs] {
            assert_min_less_than(&vals, 0.1);
            assert_max_equal_to(&vals, 1.0);
        }

        assert_min_less_than(&mutation_scales, 0.1);
        assert_max_greater_than(&mutation_scales, 10.0);
    }

    #[test]
    fn exploratory_covers_orders_of_magnitude() {
        let mut rng = StdRng::seed_from_u64(0);

        let mut crossover_probs = Vec::new();
        let mut selection_pressures = Vec::new();
        let mut mutation_probs = Vec::new();
        let mut mutation_scales = Vec::new();

        for _ in 0..100 {
            let (crossover_params, mutation_params) = create_exploratory(&mut rng);

            crossover_probs.push(crossover_params.crossover_prob);
            selection_pressures.push(crossover_params.selection_pressure);
            mutation_probs.push(mutation_params.mutation_prob);
            mutation_scales.push(mutation_params.mutation_scale);
        }

        for vals in [crossover_probs, selection_pressures, mutation_probs] {
            assert_min_less_than(&vals, 0.1);
            assert_max_equal_to(&vals, 1.0);
        }

        assert_min_less_than(&mutation_scales, 0.5);
        assert_max_greater_than(&mutation_scales, 5.0);
    }
}
