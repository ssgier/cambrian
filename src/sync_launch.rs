use crate::message::Report;
use std::panic;
use std::sync::Arc;

use crate::async_launch;
use crate::error::Error;
use crate::message::Command;
use crate::meta::AlgoParams;
use crate::meta::AsyncObjectiveFunction;
use crate::meta::MutationParams;
use crate::result::FinalReport;
use crate::termination::TerminationCriterion;
use crate::{
    meta::{CrossoverParams, ObjectiveFunction},
    spec::Spec,
};
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::executor;
use itertools::Itertools;
use tokio::runtime::{Builder, Runtime};

pub fn launch<F, T>(
    spec: Spec,
    obj_func: F,
    algo_params: AlgoParams,
    init_crossover_params: CrossoverParams,
    init_mutation_params: MutationParams,
    termination_criteria: T,
) -> Result<FinalReport, Error>
where
    F: ObjectiveFunction,
    T: IntoIterator<Item = TerminationCriterion>,
{
    let termination_criteria = termination_criteria.into_iter().collect_vec();

    let max_num_eval = termination_criteria
        .iter()
        .map(|criterion| match criterion {
            TerminationCriterion::NumObjFuncEval(max_num_eval) => max_num_eval,
        })
        .copied()
        .min();

    let (_cmd_sender, cmd_recv) = mpsc::unbounded::<Command>();
    drop(_cmd_sender);
    let (report_sender, _report_recv) = mpsc::unbounded::<Report>();

    let launch_fut = async_launch::launch(
        spec,
        AsyncObjectiveFunctionImpl::wrap(obj_func, algo_params.num_concurrent),
        algo_params,
        init_crossover_params,
        init_mutation_params,
        cmd_recv,
        report_sender,
        max_num_eval,
    );

    executor::block_on(launch_fut)
}

struct AsyncObjectiveFunctionImpl<F> {
    obj_func: Arc<F>,
    runtime: Runtime,
}

impl<F> AsyncObjectiveFunctionImpl<F>
where
    F: ObjectiveFunction,
{
    fn wrap(obj_func: F, num_worker_threads: usize) -> Self {
        Self {
            obj_func: Arc::new(obj_func),
            runtime: Builder::new_multi_thread()
                .worker_threads(num_worker_threads)
                .build()
                .unwrap(),
        }
    }
}

#[async_trait]
impl<F> AsyncObjectiveFunction for AsyncObjectiveFunctionImpl<F>
where
    F: ObjectiveFunction,
{
    async fn evaluate(&self, value: serde_json::Value) -> Option<f64> {
        let obj_func = self.obj_func.clone();

        match self
            .runtime
            .spawn(async move { obj_func.evaluate(value) })
            .await
        {
            Ok(res) => res,
            Err(join_error) if join_error.is_panic() => {
                panic::resume_unwind(join_error.into_panic())
            }
            _ => unreachable!(),
        }
    }
}
