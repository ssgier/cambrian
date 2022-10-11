use crate::meta::CrossoverParams;
use rand::{rngs::StdRng, seq::SliceRandom};
use rand_distr::{Bernoulli, Distribution};
use std::cell::RefCell;

pub struct SelectionImpl<'a> {
    rng: &'a RefCell<StdRng>,
}

impl<'a> SelectionImpl<'a> {
    pub fn new(rng: &'a RefCell<StdRng>) -> Self {
        Self { rng }
    }
}

impl Selection for SelectionImpl<'_> {
    fn select_ref<'a, T>(
        &self,
        individuals_ordered: &[&'a T],
        crossover_params: &CrossoverParams,
    ) -> &'a T {
        let rng = &mut *self.rng.borrow_mut();

        let dist = Bernoulli::new(crossover_params.selection_pressure).unwrap();
        for individual in individuals_ordered {
            if dist.sample(rng) {
                return individual;
            }
        }

        individuals_ordered.choose(rng).unwrap()
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
    use float_cmp::approx_eq;
    use rand::SeedableRng;
    use rand_distr::num_traits::ToPrimitive;

    use super::*;

    #[test]
    fn maximum_selection_pressure() {
        assert_freqs(1.0, 1.0, 0.0);
    }

    #[test]
    fn some_selection_pressure() {
        assert_freqs(0.5, 0.625, 0.375);
    }

    #[test]
    fn no_selection_pressure() {
        assert_freqs(0.0, 0.5, 0.5);
    }

    const EPSILON: f64 = 0.01;

    fn assert_freqs(selection_pressure: f64, expected_freq_0: f64, expected_freq_1: f64) {
        let rng = RefCell::new(StdRng::seed_from_u64(0));
        let sut = SelectionImpl::new(&rng);

        let individuals_ordered = [0, 1];

        let crossover_params = CrossoverParams {
            crossover_prob: 0.0,
            selection_pressure,
        };

        const N: usize = 10000;
        let mut counts = vec![0, 0];
        for _ in 0..N {
            let selected_individual = sut.select_value(&individuals_ordered, &crossover_params);
            counts[selected_individual] += 1;
        }

        let freqs: Vec<f64> = counts
            .iter()
            .map(|count| count.to_f64().unwrap() / N.to_f64().unwrap())
            .collect();

        assert!(approx_eq!(
            f64,
            freqs[0],
            expected_freq_0,
            epsilon = EPSILON
        ));

        assert!(approx_eq!(
            f64,
            freqs[1],
            expected_freq_1,
            epsilon = EPSILON
        ));
    }
}
