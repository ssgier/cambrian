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
use crate::mutation::mutate;
use crate::path::PathContext;
use crate::result::FinalReport;
use crate::spec::Spec;
use crate::value::Value;
use derivative::Derivative;
use futures::channel::oneshot::Sender;
use futures::SinkExt;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use log::trace;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BTreeSet;
use tangram_finite::FiniteF64;

struct Context<'a> {
    spec: &'a Spec,
    _algo_params: AlgoParams,
    individuals_evaled: BTreeSet<EvaluatedIndividual>,
    max_num_eval: Option<usize>,
    eval_count: usize,
    initial_value: Value,
    initial_value_job_sent: bool,
    crossover_params: CrossoverParams,
    mutation_params: MutationParams,
    crossover: Crossover<'a>,
    path: PathContext,
    rng: StdRng,
}

impl<'a> Context<'a> {
    fn new(
        spec: &'a Spec,
        algo_params: AlgoParams,
        max_num_eval: Option<usize>,
        init_crossover_params: CrossoverParams,
        init_mutation_params: MutationParams,
    ) -> Self {
        Self {
            initial_value: spec.initial_value(),
            spec,
            _algo_params: algo_params,
            individuals_evaled: BTreeSet::new(),
            max_num_eval,
            eval_count: 0,
            initial_value_job_sent: false,
            crossover_params: init_crossover_params,
            mutation_params: init_mutation_params,
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

fn finitify_obj_func_val(obj_func_val: f64) -> Result<FiniteF64, Error> {
    FiniteF64::new(obj_func_val).map_err(|_| Error::ObjFuncValMustBeFinite)
}

pub async fn start_controller(
    spec: Spec,
    algo_params: AlgoParams,
    init_crossover_params: CrossoverParams,
    init_mutation_params: MutationParams,
    mut recv: UnboundedReceiver<ControllerEvent>,
    mut report_sender: UnboundedSender<Report>,
    max_num_eval: Option<usize>,
) -> Result<FinalReport, Error> {
    let mut ctx = Context::new(
        &spec,
        algo_params,
        max_num_eval,
        init_crossover_params,
        init_mutation_params,
    );

    while let Some(event) = recv.next().await {
        match event {
            WorkerReady { eval_job_sender } => ctx.on_worker_available(eval_job_sender),
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
                    let obj_func_val = finitify_obj_func_val(obj_func_val)?;
                    trace!(
                        "Received objective function value {} for individual:\n{}",
                        obj_func_val,
                        individual.to_json()
                    );
                    ctx.process_individual_eval(obj_func_val, individual);
                } else {
                    trace!("Individual rejected:\n{}", individual.to_json())
                }

                ctx.on_worker_available(next_eval_job_sender)
            }
            TerminationCommand => break,
        }
    }

    match ctx.individuals_evaled.into_iter().next() {
        Some(evaluated_individual) => Ok(FinalReport::from_best_seen(
            evaluated_individual.obj_func_val.get(),
            evaluated_individual.individual.to_json(),
        )),
        None => Err(Error::NoIndividuals),
    }
}

impl<'a> Context<'a> {
    fn on_worker_available(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        if self
            .max_num_eval
            .map(|max_num| self.eval_count < max_num)
            .unwrap_or(true)
        {
            self.create_and_send_next_eval_job(eval_job_sender);
            self.eval_count += 1;
        }
    }

    fn create_and_send_next_eval_job(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        let individual = if self.initial_value_job_sent {
            self.create_offspring()
        } else {
            self.initial_value_job_sent = true;
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

        let result = mutate(
            self.spec,
            &crossover_result,
            &self.mutation_params,
            &mut self.path,
            &mut self.rng,
        );

        trace!(
            "Offspring created:\ncrossover result:\n{}\nmutation result:\n{}",
            crossover_result.to_json(),
            result.to_json()
        );

        result
    }

    fn process_individual_eval(&mut self, obj_func_val: FiniteF64, individual: Value) {
        self.individuals_evaled.insert(EvaluatedIndividual {
            obj_func_val,
            individual,
        });
    }
}
