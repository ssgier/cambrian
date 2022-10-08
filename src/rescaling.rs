use crate::{
    meta::{CrossoverParams, MutationParams},
    path::PathNode,
};
use std::collections::HashMap;

struct CrossoverRescaling {
    pub crossover_prob_factor: f64,
}

struct MutationRescaling {
    pub mutation_prob_factor: f64,
    pub mutation_scale_factor: f64,
}

pub struct Rescaling {
    crossover_rescaling: CrossoverRescaling,
    mutation_rescaling: MutationRescaling,
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

pub struct RescalingManager {
    factors_by_path_id: HashMap<usize, Rescaling>,
}

impl Default for RescalingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RescalingManager {
    pub fn new() -> Self {
        Self {
            factors_by_path_id: HashMap::new(),
        }
    }

    pub fn get_for_path_node(&self, path_node: &PathNode) -> &Rescaling {
        self.factors_by_path_id.get(&path_node.id).unwrap()
    }
}
