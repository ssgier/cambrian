use crate::controller::start_controller;
use crate::error::Error;
use crate::event::ControllerEvent;
use crate::message::{Command, Report};
use crate::meta::{AlgoParams, CrossoverParams, MutationParams, ObjectiveFunction};
use crate::spec::Spec;
use crate::value::Value;
use crate::worker::start_worker;
use futures::channel::mpsc;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::future;
use futures::{SinkExt, StreamExt};
use std::iter;
use std::sync::Arc;

pub async fn launch<F: ObjectiveFunction>(
    spec: Spec,
    algo_params: AlgoParams,
    init_crossover_params: CrossoverParams,
    init_mutation_params: MutationParams,
    mut cmd_recv: UnboundedReceiver<Command>,
    report_sender: UnboundedSender<Report>,
) -> Result<serde_json::Value, Error> {
    let (mut event_sender, event_recv) = mpsc::unbounded::<ControllerEvent>();

    let mut cmd_event_stream = cmd_recv.map(|cmd| match cmd {
        Command::Terminate => ControllerEvent::TerminationCommand,
    });

    cmd_event_stream.map(Ok).forward(event_sender.clone());

    let num_concurrent = algo_params.num_concurrent;

    let ctrl = start_controller(algo_params, spec, event_recv, report_sender);
    let workers = future::join_all(
        std::iter::repeat_with(|| start_worker::<F>(event_sender.clone())).take(num_concurrent),
    );

    futures::join!(ctrl, workers).0.map(|val| val.to_json())
}
