use crate::crossover::Crossover;
use crate::error::Error;
use crate::event::{
    ControllerEvent::{self, *},
    IndividualEvalJob,
};
use crate::message::Report;
use crate::meta::AlgoParams;
use crate::meta::CrossoverParams;
use crate::meta::MutationParams;
use crate::path::PathContext;
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
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BTreeSet;

struct Context<'a> {
    _spec: &'a Spec,
    _algo_params: AlgoParams,
    individuals_evaled: BTreeSet<EvaluatedIndividual>,
    initial_value: Value,
    initial_value_job_sent: bool,
    crossover_params: CrossoverParams,
    _mutation_params: MutationParams,
    crossover: Crossover<'a>,
    path: PathContext,
    rng: StdRng,
}

impl<'a> Context<'a> {
    fn new(
        spec: &'a Spec,
        algo_params: AlgoParams,
        init_crossover_params: CrossoverParams,
        init_mutation_params: MutationParams,
    ) -> Self {
        Self {
            initial_value: spec.initial_value(),
            _spec: spec,
            _algo_params: algo_params,

            individuals_evaled: BTreeSet::new(),
            initial_value_job_sent: false,
            crossover_params: init_crossover_params,
            _mutation_params: init_mutation_params,
            crossover: Crossover::new(spec),
            path: PathContext::default(),
            rng: StdRng::seed_from_u64(0),
        }
    }
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
    let mut ctx = Context::new(
        &spec,
        algo_params,
        init_crossover_params,
        init_mutation_params,
    );

    while let Some(event) = recv.next().await {
        match event {
            WorkerReady { eval_job_sender } => {
                ctx.create_and_send_next_eval_job(eval_job_sender);
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
                        ctx.process_individual_eval(obj_func_val, individual);
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

impl<'a> Context<'a> {
    fn create_and_send_next_eval_job(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        let individual = if self.initial_value_job_sent {
            self.create_offspring()
        } else {
            self.initial_value.clone()
        };

        let eval_job = IndividualEvalJob { individual };

        eval_job_sender.send(eval_job).ok();
    }

    fn create_offspring(&mut self) -> Value {
        let crossover_result = if self.individuals_evaled.is_empty() {
            self.initial_value.clone()
        } else {
            let individuals_ordered: Vec<&Value> = self
                .individuals_evaled
                .iter()
                .map(|ind| &ind.individual)
                .collect();
            self.crossover.crossover(
                &individuals_ordered,
                &self.crossover_params,
                &mut self.path,
                &mut self.rng,
            )
        };

        crossover_result // TODO, mutation
    }

    fn process_individual_eval(&mut self, obj_func_val: FiniteF64, individual: Value) {
        self.individuals_evaled.insert(EvaluatedIndividual {
            obj_func_val,
            individual,
        });
    }
}
