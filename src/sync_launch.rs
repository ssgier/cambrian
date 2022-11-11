use crate::async_launch;
use crate::detailed_report::DetailedReportItem;
use crate::error::Error;
use crate::message::Command;
use crate::meta::AlgoConfig;
use crate::meta::AsyncObjectiveFunction;
use crate::result::FinalReport;
use crate::termination;
use crate::termination::TerminationCriterion;
use crate::{meta::ObjectiveFunction, spec::Spec};
use async_channel;
use async_trait::async_trait;
use ctrlc;
use futures::channel::mpsc;
use futures::channel::mpsc::UnboundedReceiver;
use futures::executor;
use futures::future::Either;
use futures::pin_mut;
use futures::select;
use futures::sink::SinkExt;
use futures::FutureExt;
use futures::StreamExt;
use futures_timer::Delay;
use log::info;
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::runtime;

pub fn launch<F, T>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    termination_criteria: T,
    explicit_init_value_json: Option<serde_json::Value>,
    in_process_computation: bool,
    detailed_report_path: Option<&PathBuf>,
) -> Result<FinalReport, Error>
where
    F: ObjectiveFunction,
    T: IntoIterator<Item = TerminationCriterion>,
{
    launch_with_async_obj_func(
        spec,
        AsyncObjectiveFunctionImpl::wrap(obj_func),
        algo_config,
        termination_criteria,
        explicit_init_value_json,
        in_process_computation,
        detailed_report_path,
    )
}

async fn stream_detailed_report_items_to_file(
    file_path: Option<&PathBuf>,
    mut item_receiver: UnboundedReceiver<DetailedReportItem>,
) -> Result<(), Error> {
    if let Some(file_path) = file_path {
        let mut detailed_report_file = File::create(file_path).await.map_err(|err| {
            Error::UnableToCreateDetailedReportFile {
                path: file_path.to_owned(),
                source: err,
            }
        })?;

        detailed_report_file
            .write_all(DetailedReportItem::get_csv_header_row().as_bytes())
            .await?;

        while let Some(item) = item_receiver.next().await {
            detailed_report_file
                .write_all(item.to_csv_row().as_bytes())
                .await?;
        }
    } else {
        item_receiver
            .map(Ok)
            .forward(futures::sink::drain())
            .await
            .unwrap();
    }

    Ok(())
}

pub fn launch_with_async_obj_func<F, T>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    termination_criteria: T,
    explicit_init_value_json: Option<serde_json::Value>,
    in_process_computation: bool,
    detailed_report_path: Option<&PathBuf>,
) -> Result<FinalReport, Error>
where
    F: AsyncObjectiveFunction,
    T: IntoIterator<Item = TerminationCriterion>,
{
    let mut runtime_builder = if in_process_computation {
        let mut builder = runtime::Builder::new_multi_thread();
        builder.worker_threads(algo_config.num_concurrent);
        builder
    } else {
        runtime::Builder::new_current_thread()
    };

    let termination_criteria = termination::compile(termination_criteria)?;

    let (mut cmd_sender, cmd_recv) = mpsc::unbounded::<Command>();
    let (detailed_report_sender, detailed_report_recv) = mpsc::unbounded::<DetailedReportItem>();

    let launch_fut = async_launch::launch(
        spec,
        obj_func,
        algo_config,
        cmd_recv,
        detailed_report_sender,
        termination_criteria.max_num_obj_func_eval,
        termination_criteria.target_obj_func_val,
        explicit_init_value_json,
    );

    let detailed_reporting_fut =
        stream_detailed_report_items_to_file(detailed_report_path, detailed_report_recv);

    if termination_criteria.terminate_on_signal {
        let mut sender_for_handler = cmd_sender.clone();
        ctrlc::set_handler(move || {
            info!("Received signal, will terminate after collecting result");
            executor::block_on(sender_for_handler.send(Command::Terminate)).ok();
        })?;
    }

    runtime_builder
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let timeout_fut = if let Some(terminate_after) = termination_criteria.terminate_after {
                Either::Left(Delay::new(terminate_after).fuse())
            } else {
                Either::Right(futures::future::pending())
            };

            let launch_fut = launch_fut.fuse();
            let detailed_reporting_fut = detailed_reporting_fut.fuse();
            pin_mut!(timeout_fut, launch_fut, detailed_reporting_fut);

            loop {
                select! {
                    () = &mut timeout_fut => {
                        info!("Abort time reached");
                        cmd_sender.send(Command::Terminate).await.unwrap();
                    }
                    res = &mut launch_fut => {
                        detailed_reporting_fut.await?;
                        return res;
                    }
                    res = &mut detailed_reporting_fut => {
                        res?;
                    }
                }
            }
        })
}

struct AsyncObjectiveFunctionImpl<F> {
    obj_func: Arc<F>,
}

impl<F> AsyncObjectiveFunctionImpl<F>
where
    F: ObjectiveFunction,
{
    fn wrap(obj_func: F) -> Self {
        Self {
            obj_func: Arc::new(obj_func),
        }
    }
}

#[async_trait]
impl<F> AsyncObjectiveFunction for AsyncObjectiveFunctionImpl<F>
where
    F: ObjectiveFunction,
{
    async fn evaluate(
        &self,
        value: serde_json::Value,
        abort_sig_recv: async_channel::Receiver<()>,
    ) -> Result<Option<f64>, Error> {
        let obj_func = self.obj_func.clone();

        let join_handle = tokio::spawn(async move { obj_func.evaluate(value) });

        tokio::select! {
            _ = abort_sig_recv.recv() => return Ok(None),
            join_result = join_handle => match join_result {
            Ok(res) => Ok(res),
            Err(join_error) if join_error.is_panic() => {
                panic::resume_unwind(join_error.into_panic())
            }
            _ => unreachable!(),
            }
        }
    }
}
