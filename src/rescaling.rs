use crate::meta::{CrossoverParams, MutationParams};

pub struct CrossoverRescaling {
    pub crossover_prob_factor: f64,
}

pub struct MutationRescaling {
    pub mutation_prob_factor: f64,
    pub mutation_scale_factor: f64,
}

#[derive(Default)]
pub struct Rescaling {
    pub crossover_rescaling: CrossoverRescaling,
    pub mutation_rescaling: MutationRescaling,
}

impl Default for CrossoverRescaling {
    fn default() -> Self {
        Self {
            crossover_prob_factor: 1.0,
        }
    }
}

impl Default for MutationRescaling {
    fn default() -> Self {
        Self {
            mutation_prob_factor: 1.0,
            mutation_scale_factor: 1.0,
        }
    }
}

impl Rescaling {
    pub fn rescale_crossover(&self, pre: &CrossoverParams) -> CrossoverParams {
        CrossoverParams {
            crossover_prob: pre.crossover_prob * self.crossover_rescaling.crossover_prob_factor,
        }
    }

    pub fn rescale_mutation(&self, pre: &MutationParams) -> MutationParams {
        MutationParams {
            mutation_prob: pre.mutation_prob * self.mutation_rescaling.mutation_prob_factor,
            mutation_scale: pre.mutation_scale * self.mutation_rescaling.mutation_scale_factor,
        }
    }
}

#[derive(Default)]
pub struct RescalingContext {
    pub current_rescaling: Rescaling,
}
