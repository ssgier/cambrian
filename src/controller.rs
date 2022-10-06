use crate::error::Error;
use crate::event::{
    ControllerEvent::{self, *},
    IndividualEvalJob,
};
use crate::message::{Command, Report};
use crate::meta::AlgoParams;
use crate::spawn;
use crate::spec::Spec;
use crate::value::Value;
use derivative::Derivative;
use finite::FiniteF64;
use futures::channel::oneshot::Sender;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use std::collections::BTreeSet;

struct Context {
    algo_params: AlgoParams,
    spec: Spec,
    individuals_evaled: BTreeSet<EvaluatedIndividual>,
    initial_value: Value,
    initial_value_job_sent: bool,
}

#[derive(Derivative)]
#[derivative(Ord, Eq, PartialOrd, PartialEq)]
struct EvaluatedIndividual {
    obj_func_val: FiniteF64,
    #[derivative(Ord = "ignore", PartialOrd = "ignore", PartialEq = "ignore")]
    individual: Value,
}

pub async fn start_controller(
    algo_params: AlgoParams,
    spec: Spec,
    mut recv: UnboundedReceiver<ControllerEvent>,
    _report_sender: UnboundedSender<Report>,
) -> Result<Value, Error> {
    let mut ctx = Context {
        algo_params,
        individuals_evaled: BTreeSet::new(),
        initial_value: spawn::initial_value(&spec),
        initial_value_job_sent: false,
        spec,
    };

    while let Some(event) = recv.next().await {
        match event {
            WorkerReady { eval_job_sender } => ctx.create_and_send_next_eval_job(eval_job_sender),
            IndividualEvalCompleted {
                obj_func_val,
                individual,
                next_eval_job_sender,
            } => {
                if let Some(obj_func_val) = obj_func_val {
                    if let Some(obj_func_val) = FiniteF64::new(obj_func_val) {
                        ctx.individuals_evaled.insert(EvaluatedIndividual {
                            obj_func_val,
                            individual,
                        });
                    } else {
                        return Err(Error::ObjFuncValMustBeFinite);
                    }
                }

                ctx.create_and_send_next_eval_job(next_eval_job_sender);
            }
            TerminationCommand => break,
        }
    }

    match ctx.individuals_evaled.into_iter().next() {
        Some(evaluated_individual) => Ok(evaluated_individual.individual),
        None => Err(Error::NoIndividuals),
    }
}

impl Context {
    fn create_and_send_next_eval_job(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        let individual = if self.initial_value_job_sent {
            self.createOffspring()
        } else {
            self.initial_value.clone()
        };

        let eval_job = IndividualEvalJob { individual };

        eval_job_sender.send(eval_job);
    }

    fn createOffspring(&self) -> Value {
        self.initial_value.clone()
    }
}
