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
pub trait ObjectiveFunction {
    async fn evaluate(value: &serde_json::Value) -> Option<f64>;
}
