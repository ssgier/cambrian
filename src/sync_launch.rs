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
use futures::channel::mpsc::Receiver;
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

const CHANNEL_BUF_SIZE: usize = 256;

pub struct DetailedReportingFileInfo {
    pub detailed_report_file_path: PathBuf,
    pub best_seen_file_path: PathBuf,
}

pub fn launch<F, T>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    termination_criteria: T,
    explicit_init_value_json: Option<serde_json::Value>,
    in_process_computation: bool,
    detailed_reporting_file_info: Option<&DetailedReportingFileInfo>,
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
        detailed_reporting_file_info,
    )
}

async fn write_best_seen_file(
    value: &serde_json::Value,
    file_info: &DetailedReportingFileInfo,
) -> Result<(), Error> {
    let mut best_seen_file = File::create(&file_info.best_seen_file_path)
        .await
        .map_err(|err| Error::UnableToCreateDetailedReportingFile {
            path: file_info.best_seen_file_path.to_owned(),
            source: err,
        })?;

    best_seen_file
        .write_all(value.to_string().as_bytes())
        .await?;

    Ok(())
}

async fn handle_detailed_report_items(
    detailed_reporting_file_info: Option<&DetailedReportingFileInfo>,
    mut item_receiver: Receiver<DetailedReportItem>,
) -> Result<(), Error> {
    if let Some(file_info) = detailed_reporting_file_info {
        let mut detailed_report_file = File::create(&file_info.detailed_report_file_path)
            .await
            .map_err(|err| Error::UnableToCreateDetailedReportingFile {
                path: file_info.detailed_report_file_path.to_owned(),
                source: err,
            })?;

        detailed_report_file
            .write_all(DetailedReportItem::get_csv_header_row().as_bytes())
            .await?;

        let mut best_seen: Option<DetailedReportItem> = None;

        while let Some(item) = item_receiver.next().await {
            detailed_report_file
                .write_all(item.to_csv_row().as_bytes())
                .await?;

            if let Some(item_obj_func_val) = item.obj_func_val {
                let new_best_seen = if let Some(ref best_seen) = best_seen {
                    let best_obj_func_val = best_seen.obj_func_val.unwrap();
                    item_obj_func_val < best_obj_func_val
                } else {
                    true
                };

                if new_best_seen {
                    write_best_seen_file(&item.input_val, file_info).await?;
                    best_seen = Some(item);
                }
            };
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
    detailed_reporting_file_info: Option<&DetailedReportingFileInfo>,
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

    let (mut cmd_sender, cmd_recv) = mpsc::channel::<Command>(CHANNEL_BUF_SIZE);
    let (detailed_report_sender, detailed_report_recv) =
        mpsc::channel::<DetailedReportItem>(CHANNEL_BUF_SIZE);

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
        handle_detailed_report_items(detailed_reporting_file_info, detailed_report_recv);

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
