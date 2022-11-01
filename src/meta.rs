use async_trait::async_trait;

use crate::error::Error;
use async_channel::Receiver;

#[derive(Debug, Clone)]
pub struct CrossoverParams {
    // TODO: sanitize on instantiation
    pub crossover_prob: f64,
    pub selection_pressure: f64,
}

#[derive(Debug, Clone)]
pub struct MutationParams {
    pub mutation_prob: f64,
    pub mutation_scale: f64,
}

#[derive(Debug, Clone)]
pub struct AlgoConfig {
    pub is_stochastic: bool,
    pub num_concurrent: usize,
    pub max_population_size: usize,
    pub init_crossover_params: CrossoverParams,
    pub init_mutation_params: MutationParams,
}

#[async_trait]
pub trait AsyncObjectiveFunction: Sync {
    async fn evaluate(
        &self,
        value: serde_json::Value,
        abort_signal_recv: Receiver<()>,
    ) -> Result<Option<f64>, Error>;
}

pub trait ObjectiveFunction: Sync + Send + 'static {
    fn evaluate(&self, value: serde_json::Value) -> Option<f64>;
}

pub struct ObjectiveFunctionImpl<F> {
    obj_func: F,
}

impl<F> ObjectiveFunction for ObjectiveFunctionImpl<F>
where
    F: Fn(serde_json::Value) -> Option<f64> + Send + Sync + 'static,
{
    fn evaluate(&self, value: serde_json::Value) -> Option<f64> {
        (self.obj_func)(value)
    }
}

pub fn make_obj_func<F>(f: F) -> ObjectiveFunctionImpl<F>
where
    F: Fn(serde_json::Value) -> Option<f64>,
{
    ObjectiveFunctionImpl { obj_func: f }
}

pub struct AlgoConfigBuilder {
    is_stochastic: Option<bool>,
    num_concurrent: Option<usize>,
    max_population_size: Option<usize>,
    init_crossover_params: Option<CrossoverParams>,
    init_mutation_params: Option<MutationParams>,
}

const DEFAULT_POPULATION_SIZE: usize = 20;

const DEFAULT_INIT_CROSSOVER_PARAMS: CrossoverParams = CrossoverParams {
    crossover_prob: 0.75,
    selection_pressure: 0.2,
};

const DEFAULT_INIT_MUTATION_PARAMS: MutationParams = MutationParams {
    mutation_prob: 0.3,
    mutation_scale: 1.0,
};

impl Default for AlgoConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AlgoConfigBuilder {
    pub fn is_stochastic(&mut self, is_stochastic: bool) -> &mut Self {
        self.is_stochastic = Some(is_stochastic);
        self
    }

    pub fn num_concurrent(&mut self, num_concurrent: usize) -> &mut Self {
        self.num_concurrent = Some(num_concurrent);
        self
    }

    pub fn max_population_size(&mut self, max_population_size: usize) -> &mut Self {
        self.max_population_size = Some(max_population_size);
        self
    }

    pub fn init_crossover_params(&mut self, init_crossover_params: CrossoverParams) -> &mut Self {
        self.init_crossover_params = Some(init_crossover_params);
        self
    }

    pub fn init_mutation_params(&mut self, init_mutation_params: MutationParams) -> &mut Self {
        self.init_mutation_params = Some(init_mutation_params);
        self
    }

    pub fn new() -> Self {
        Self {
            is_stochastic: None,
            num_concurrent: None,
            max_population_size: None,
            init_crossover_params: None,
            init_mutation_params: None,
        }
    }

    pub fn build(&mut self) -> AlgoConfig {
        AlgoConfig {
            is_stochastic: self.is_stochastic.unwrap_or(false),
            num_concurrent: self.num_concurrent.unwrap_or(1),
            max_population_size: self.max_population_size.unwrap_or(DEFAULT_POPULATION_SIZE),
            init_crossover_params: self
                .init_crossover_params
                .clone()
                .unwrap_or(DEFAULT_INIT_CROSSOVER_PARAMS),
            init_mutation_params: self
                .init_mutation_params
                .clone()
                .unwrap_or(DEFAULT_INIT_MUTATION_PARAMS),
        }
    }
}
