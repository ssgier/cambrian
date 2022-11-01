use crate::crossover::Crossover;
use crate::mutation;
use crate::value::Value;
use crate::{
    meta::{CrossoverParams, MutationParams},
    path::PathContext,
    spec::Spec,
};
use log::{info, trace};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BTreeMap;
use tangram_finite::FiniteF64;

#[derive(Ord, Eq, PartialEq, PartialOrd, Clone, Debug)]
struct IndividualOrderingKey {
    obj_func_val: FiniteF64,
    id: usize,
}

impl IndividualOrderingKey {
    fn new(id: usize, obj_func_val: FiniteF64) -> Self {
        Self { obj_func_val, id }
    }
}

pub struct AlgoContext {
    spec: Spec,
    _is_stochastic: bool,
    max_population_size: usize,
    crossover_params: CrossoverParams,
    mutation_params: MutationParams,

    individuals_evaled: BTreeMap<IndividualOrderingKey, Value>,
    initial_value: Option<Value>,
    crossover: Crossover,
    path_ctx: PathContext,
    rng: StdRng,
    next_id: usize,
}

impl AlgoContext {
    pub fn new(
        spec: Spec,
        is_stochastic: bool,
        max_population_size: usize,
        crossover_params: CrossoverParams,
        mutation_params: MutationParams,
    ) -> Self {
        Self {
            spec,
            _is_stochastic: is_stochastic,
            max_population_size,
            crossover_params,
            mutation_params,
            individuals_evaled: BTreeMap::new(),
            initial_value: None,
            crossover: Crossover::new(),
            path_ctx: PathContext::default(),
            rng: StdRng::seed_from_u64(0),
            next_id: 0,
        }
    }
}

pub struct IdentifiableIndividual {
    id: usize,
    pub value: Value,
}

impl IdentifiableIndividual {
    fn new(id: usize, value: Value) -> Self {
        Self { id, value }
    }
}

pub struct EvaluatedIndividual {
    pub identifiable_individual: IdentifiableIndividual,
    pub obj_func_val: Option<FiniteF64>,
}

impl EvaluatedIndividual {
    pub fn new(
        identifiable_individual: IdentifiableIndividual,
        obj_func_val: Option<FiniteF64>,
    ) -> Self {
        Self {
            identifiable_individual,
            obj_func_val,
        }
    }
}

impl AlgoContext {
    fn make_id(&mut self) -> usize {
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    pub fn create_individual(&mut self) -> IdentifiableIndividual {
        let value = if self.initial_value.is_none() {
            self.initial_value = Some(self.spec.initial_value());
            self.initial_value.clone().unwrap()
        } else {
            self.create_offspring()
        };

        let individual_id = self.make_id();

        IdentifiableIndividual::new(individual_id, value)
    }

    fn create_offspring(&mut self) -> Value {
        let crossover_result = if self.individuals_evaled.is_empty() {
            self.initial_value.clone().unwrap()
        } else {
            let individuals_ordered: Vec<&Value> =
                self.individuals_evaled.iter().map(|ind| ind.1).collect();
            self.crossover.crossover(
                &self.spec,
                &individuals_ordered,
                &self.crossover_params,
                &mut self.path_ctx,
                &mut self.rng,
            )
        };

        let result = mutation::mutate(
            &self.spec,
            &crossover_result,
            &self.mutation_params,
            &mut self.path_ctx,
            &mut self.rng,
        );

        trace!(
            "Offspring created:\ncrossover result:\n{}\nmutation result:\n{}",
            crossover_result.to_json(),
            result.to_json()
        );

        result
    }

    pub fn process_individual_eval(&mut self, evaled_individual: EvaluatedIndividual) {
        if let Some(obj_func_val) = evaled_individual.obj_func_val {
            let ordering_key = IndividualOrderingKey::new(
                evaled_individual.identifiable_individual.id,
                obj_func_val,
            );
            let new_best = self
                .individuals_evaled
                .keys()
                .next()
                .map(|ordering_key| obj_func_val < ordering_key.obj_func_val)
                .unwrap_or(true);

            if new_best {
                info!("New best objective function value: {}", obj_func_val);
            }

            self.individuals_evaled.insert(
                ordering_key,
                evaled_individual.identifiable_individual.value,
            );

            while self.individuals_evaled.len() > self.max_population_size {
                let last_ordering_key: IndividualOrderingKey =
                    self.individuals_evaled.keys().next_back().unwrap().clone();
                self.individuals_evaled.remove(&last_ordering_key);
            }
        }
    }

    pub fn peek_best_seen_value(&self) -> Option<f64> {
        self.individuals_evaled
            .iter()
            .next()
            .map(|(ordering_key, _)| ordering_key.obj_func_val.get())
    }

    pub fn best_seen(self) -> Option<(f64, Value)> {
        self.individuals_evaled
            .into_iter()
            .map(|(ordering_key, value)| (ordering_key.obj_func_val.get(), value))
            .next()
    }
}
