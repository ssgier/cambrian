use std::sync::Arc;

use crate::controller::start_controller;
use crate::error::Error;
use crate::event::ControllerEvent;
use crate::message::{Command, Report};
use crate::meta::{AlgoConfig, AsyncObjectiveFunction};
use crate::result::FinalReport;
use crate::spec::Spec;
use crate::worker::start_worker;
use futures::channel::mpsc;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::select;
use futures::StreamExt;
use futures::{future, pin_mut, FutureExt};

pub async fn launch<F: AsyncObjectiveFunction>(
    spec: Spec,
    obj_func: F,
    algo_config: AlgoConfig,
    cmd_recv: UnboundedReceiver<Command>,
    report_sender: UnboundedSender<Report>,
    max_num_eval: Option<usize>,
    target_obj_func_val: Option<f64>,
) -> Result<FinalReport, Error> {
    let (event_sender, event_recv) = mpsc::unbounded::<ControllerEvent>();

    let cmd_event_stream = cmd_recv.map(|cmd| match cmd {
        Command::Terminate => ControllerEvent::TerminationCommand,
    });

    let cmd_event_stream = cmd_event_stream
        .map(Ok)
        .forward(event_sender.clone())
        .fuse();

    let num_concurrent = algo_config.num_concurrent;

    let ctrl = start_controller(
        spec,
        algo_config,
        event_recv,
        report_sender,
        max_num_eval,
        target_obj_func_val,
    )
    .fuse();

    let obj_func = Arc::new(obj_func);
    let mut workers = future::select_all(
        std::iter::repeat_with(|| {
            let fut = start_worker(obj_func.clone(), event_sender.clone()).fuse();
            Box::pin(fut)
        })
        .take(num_concurrent),
    )
    .fuse();

    pin_mut!(ctrl, cmd_event_stream);

    loop {
        select! {
            (worker_result, _, remaining_workers) = workers => {
                worker_result?;
                if !remaining_workers.is_empty() {
                    workers = future::select_all(remaining_workers).fuse();
                }
            },
            result = ctrl => return result,
            _ = cmd_event_stream => return Err(Error::ClientHungUp),
        };
    }
}
