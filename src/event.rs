use crate::error::Error;
use crate::value::Value;
use finite::FiniteF64;
use futures::channel::oneshot::{self, Receiver, Sender};

pub struct IndividualEvalJob {
    pub individual: Value,
}

pub enum ControllerEvent {
    WorkerReady {
        eval_job_sender: Sender<IndividualEvalJob>,
    },
    IndividualEvalCompleted {
        obj_func_val: Option<f64>,
        individual: Value,
        next_eval_job_sender: Sender<IndividualEvalJob>,
    },
    TerminationCommand,
}
