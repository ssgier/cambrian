use std::sync::Arc;
use crate::value::Value;
use futures::channel::oneshot::Sender;

pub struct IndividualEvalJob {
    pub individual: Arc<Value>,
    pub individual_id: usize,
}

pub enum ControllerEvent {
    WorkerReady {
        eval_job_sender: Sender<IndividualEvalJob>,
    },
    IndividualEvalCompleted {
        obj_func_val: Option<f64>,
        individual_id: usize,
        next_eval_job_sender: Sender<IndividualEvalJob>,
    },
    TerminationCommand,
}
