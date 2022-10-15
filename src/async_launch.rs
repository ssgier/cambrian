use std::sync::Arc;

use crate::controller::start_controller;
use crate::error::Error;
use crate::event::ControllerEvent;
use crate::message::{Command, Report};
use crate::meta::{AlgoParams, AsyncObjectiveFunction, CrossoverParams, MutationParams};
use crate::result::FinalReport;
use crate::spec::Spec;
use crate::worker::start_worker;
use futures::channel::mpsc;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future;
use futures::StreamExt;

#[allow(clippy::too_many_arguments)]
pub async fn launch<F: AsyncObjectiveFunction>(
    spec: Spec,
    obj_func: F,
    algo_params: AlgoParams,
    init_crossover_params: CrossoverParams,
    init_mutation_params: MutationParams,
    cmd_recv: UnboundedReceiver<Command>,
    report_sender: UnboundedSender<Report>,
    max_num_eval: Option<usize>,
) -> Result<FinalReport, Error> {
    let (event_sender, event_recv) = mpsc::unbounded::<ControllerEvent>();

    let cmd_event_stream = cmd_recv.map(|cmd| match cmd {
        Command::Terminate => ControllerEvent::TerminationCommand,
    });

    let cmd_event_stream = cmd_event_stream.map(Ok).forward(event_sender.clone());

    let num_concurrent = algo_params.num_concurrent;

    let ctrl = start_controller(
        spec,
        algo_params,
        init_crossover_params,
        init_mutation_params,
        event_recv,
        report_sender,
        max_num_eval,
    );

    let obj_func = Arc::new(obj_func);
    let workers = future::join_all(
        std::iter::repeat_with(|| start_worker(obj_func.clone(), event_sender.clone()))
            .take(num_concurrent),
    );

    futures::join!(ctrl, workers, cmd_event_stream).0
}
