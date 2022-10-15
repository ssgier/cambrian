use async_trait::async_trait;

#[derive(Debug)]
pub struct CrossoverParams {
    // TODO: sanitize on instantiation
    pub crossover_prob: f64,
    pub selection_pressure: f64,
}

#[derive(Debug)]
pub struct MutationParams {
    pub mutation_prob: f64,
    pub mutation_scale: f64,
}

#[derive(Debug)]
pub struct AlgoParams {
    pub is_stochastic: bool,
    pub num_concurrent: usize,
}

#[async_trait]
pub trait AsyncObjectiveFunction: Sync {
    async fn evaluate(&self, value: serde_json::Value) -> Option<f64>;
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
