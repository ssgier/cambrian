use async_trait::async_trait;

use crate::error::Error;
use async_channel::Receiver;
use enum_display_derive::Display;
use std::fmt::Display;

#[derive(Clone, Debug)]
pub struct MetaParamsWrapper {
    pub source: MetaParamsSource,
    pub crossover_params: CrossoverParams,
    pub mutation_params: MutationParams,
}

impl MetaParamsWrapper {
    pub fn new(
        source: MetaParamsSource,
        crossover_params: CrossoverParams,
        mutation_params: MutationParams,
    ) -> Self {
        Self {
            source,
            crossover_params,
            mutation_params,
        }
    }
}

#[derive(Clone, Debug, Display)]
pub enum MetaParamsSource {
    Exploratory,
    Selected,
    SelectedAndMutated,
    Override,
}

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
}

const DEFAULT_IND_SAMPLE_SIZE: usize = 1;

const DEFAULT_QUANTILE: f64 = 1.0;

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

    pub fn new() -> Self {
        Self {
            individual_sample_size: None,
            obj_func_val_quantile: None,
            num_concurrent: None,
        }
    }

    pub fn build(&mut self) -> Result<AlgoConfig, Error> {
        let algo_config = AlgoConfig {
            individual_sample_size: self
                .individual_sample_size
                .unwrap_or(DEFAULT_IND_SAMPLE_SIZE),
            obj_func_val_quantile: self.obj_func_val_quantile.unwrap_or(DEFAULT_QUANTILE),
            num_concurrent: self.num_concurrent.unwrap_or(1),
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

        Ok(algo_config)
    }
}
