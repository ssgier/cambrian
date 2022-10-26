use crate::async_launch;
use crate::error::Error;
use crate::message::Command;
use crate::message::Report;
use crate::meta::AlgoConfig;
use crate::meta::AsyncObjectiveFunction;
use crate::result::FinalReport;
use crate::termination;
use crate::termination::TerminationCriterion;
use crate::{meta::ObjectiveFunction, spec::Spec};
use async_trait::async_trait;
use ctrlc;
use futures::channel::mpsc;
use futures::executor;
use futures::pin_mut;
use futures::select;
use futures::sink::SinkExt;
use futures::FutureExt;
use futures_timer::Delay;
use log::info;
use std::panic;
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

pub fn launch<F, T>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    termination_criteria: T,
) -> Result<FinalReport, Error>
where
    F: ObjectiveFunction,
    T: IntoIterator<Item = TerminationCriterion>,
{
    launch_with_async_obj_func(
        spec,
        AsyncObjectiveFunctionImpl::wrap(obj_func, algo_config.num_concurrent),
        algo_config,
        termination_criteria,
    )
}

pub fn launch_with_async_obj_func<F, T>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    termination_criteria: T,
) -> Result<FinalReport, Error>
where
    F: AsyncObjectiveFunction,
    T: IntoIterator<Item = TerminationCriterion>,
{
    let termination_criteria = termination::compile(termination_criteria)?;

    let (mut cmd_sender, cmd_recv) = mpsc::unbounded::<Command>();
    let (report_sender, _report_recv) = mpsc::unbounded::<Report>();

    let launch_fut = async_launch::launch(
        spec,
        obj_func,
        algo_config,
        cmd_recv,
        report_sender,
        termination_criteria.max_num_obj_func_eval,
        termination_criteria.target_obj_func_val,
    );

    if termination_criteria.terminate_on_signal {
        let mut sender_for_handler = cmd_sender.clone();
        ctrlc::set_handler(move || {
            info!("Received signal, will terminate after collecting result");
            executor::block_on(sender_for_handler.send(Command::Terminate)).ok();
        })?;
    }

    executor::block_on(async {
        match termination_criteria.terminate_after {
            None => launch_fut.await,
            Some(terminate_after) => {
                let timeout_fut = Delay::new(terminate_after).fuse();
                let launch_fut = launch_fut.fuse();
                pin_mut!(timeout_fut, launch_fut);

                loop {
                    select! {
                        () = &mut timeout_fut => {
                            info!("Abort time reached");
                            cmd_sender.send(Command::Terminate).await.ok();
                        }
                        report = launch_fut => {
                            return report;
                        }
                    }
                }
            }
        }
    })
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
    async fn evaluate(&self, value: serde_json::Value) -> Result<Option<f64>, Error> {
        let obj_func = self.obj_func.clone();

        match self
            .runtime
            .spawn(async move { obj_func.evaluate(value) })
            .await
        {
            Ok(res) => Ok(res),
            Err(join_error) if join_error.is_panic() => {
                panic::resume_unwind(join_error.into_panic())
            }
            _ => unreachable!(),
        }
    }
}
