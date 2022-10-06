use crate::error::Error;
use crate::event::{
    ControllerEvent::{self, *},
    IndividualEvalJob,
};
use crate::message::Report;
use crate::meta::AlgoParams;
use crate::meta::CrossoverParams;
use crate::meta::MutationParams;
use crate::spawn;
use crate::spec::Spec;
use crate::value::Value;
use derivative::Derivative;
use finite::FiniteF64;
use futures::channel::oneshot::Sender;
use futures::SinkExt;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use std::collections::BTreeSet;

struct Context {
    spec: Spec,
    algo_params: AlgoParams,
    individuals_evaled: BTreeSet<EvaluatedIndividual>,
    initial_value: Value,
    initial_value_job_sent: bool,
    crossover_params: CrossoverParams,
    mutation_params: MutationParams,
}

#[derive(Derivative)]
#[derivative(Ord, Eq, PartialOrd, PartialEq)]
struct EvaluatedIndividual {
    obj_func_val: FiniteF64,
    #[derivative(Ord = "ignore", PartialOrd = "ignore", PartialEq = "ignore")]
    individual: Value,
}

pub async fn start_controller(
    spec: Spec,
    algo_params: AlgoParams,
    init_crossover_params: CrossoverParams,
    init_mutation_params: MutationParams,
    mut recv: UnboundedReceiver<ControllerEvent>,
    mut report_sender: UnboundedSender<Report>,
) -> Result<Value, Error> {
    let mut ctx = Context {
        initial_value: spawn::initial_value(&spec),
        spec,
        algo_params,
        individuals_evaled: BTreeSet::new(),
        initial_value_job_sent: false,
        crossover_params: init_crossover_params,
        mutation_params: init_mutation_params,
    };

    while let Some(event) = recv.next().await {
        match event {
            WorkerReady { eval_job_sender } => {
                ctx.create_and_send_next_eval_job(eval_job_sender).await;
            }
            IndividualEvalCompleted {
                obj_func_val,
                individual,
                next_eval_job_sender,
            } => {
                report_sender
                    .send(Report::IndividualEvalCompleted {
                        obj_func_val,
                        individual: individual.to_json(),
                    })
                    .await
                    .ok();

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

                ctx.create_and_send_next_eval_job(next_eval_job_sender)
                    .await;
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
    async fn create_and_send_next_eval_job(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        let individual = if self.initial_value_job_sent {
            self.create_offspring()
        } else {
            self.initial_value.clone()
        };

        let eval_job = IndividualEvalJob { individual };

        eval_job_sender.send(eval_job).ok();
    }

    fn create_offspring(&self) -> Value {
        self.initial_value.clone()
    }
}
