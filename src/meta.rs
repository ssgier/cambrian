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
    pub individual_sample_size: usize,
    pub obj_func_val_quantile: f64,
    pub num_concurrent: usize,
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
    individual_sample_size: Option<usize>,
    obj_func_val_quantile: Option<f64>,
    num_concurrent: Option<usize>,
    init_crossover_params: Option<CrossoverParams>,
    init_mutation_params: Option<MutationParams>,
}

const DEFAULT_IND_SAMPLE_SIZE: usize = 1;

const DEFAULT_QUANTILE: f64 = 1.0;

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
    pub fn individual_sample_size(&mut self, individual_sample_size: usize) -> &mut Self {
        self.individual_sample_size = Some(individual_sample_size);
        self
    }

    pub fn obj_func_val_quantile(&mut self, obj_func_val_quantile: f64) -> &mut Self {
        self.obj_func_val_quantile = Some(obj_func_val_quantile);
        self
    }

    pub fn num_concurrent(&mut self, num_concurrent: usize) -> &mut Self {
        self.num_concurrent = Some(num_concurrent);
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
            individual_sample_size: None,
            obj_func_val_quantile: None,
            num_concurrent: None,
            init_crossover_params: None,
            init_mutation_params: None,
        }
    }

    pub fn build(&mut self) -> Result<AlgoConfig, Error> {
        let algo_config = AlgoConfig {
            individual_sample_size: self
                .individual_sample_size
                .unwrap_or(DEFAULT_IND_SAMPLE_SIZE),
            obj_func_val_quantile: self.obj_func_val_quantile.unwrap_or(DEFAULT_QUANTILE),
            num_concurrent: self.num_concurrent.unwrap_or(1),
            init_crossover_params: self
                .init_crossover_params
                .clone()
                .unwrap_or(DEFAULT_INIT_CROSSOVER_PARAMS),
            init_mutation_params: self
                .init_mutation_params
                .clone()
                .unwrap_or(DEFAULT_INIT_MUTATION_PARAMS),
        };

        if algo_config.obj_func_val_quantile < 0.0 || algo_config.obj_func_val_quantile > 1.0 {
            return Err(Error::InvalidQuantile);
        }

        if algo_config.individual_sample_size == 0 {
            return Err(Error::ZeroSampleSize);
        }

        if algo_config.num_concurrent == 0 {
            return Err(Error::ZeroNumConcurrent);
        }

        if algo_config.init_crossover_params.crossover_prob < 0.0
            || algo_config.init_crossover_params.crossover_prob > 1.0
        {
            return Err(Error::InvalidCrossoverProbability);
        }

        if algo_config.init_crossover_params.selection_pressure < 0.0
            || algo_config.init_crossover_params.selection_pressure > 1.0
        {
            return Err(Error::InvalidSelectionPressure);
        }

        if algo_config.init_mutation_params.mutation_prob < 0.0
            || algo_config.init_mutation_params.mutation_prob > 1.0
        {
            return Err(Error::InvalidMutationProbability);
        }

        if algo_config.init_mutation_params.mutation_scale < 0.0
            || algo_config.init_mutation_params.mutation_scale > 1.0
        {
            return Err(Error::InvalidMutationScale);
        }

        Ok(algo_config)
    }
}
