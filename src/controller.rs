use crate::crossover::Crossover;
use crate::error::Error;
use crate::event::{
    ControllerEvent::{self, *},
    IndividualEvalJob,
};
use crate::message::Report;
use crate::meta::AlgoConfig;
use crate::meta::CrossoverParams;
use crate::meta::MutationParams;
use crate::mutation::mutate;
use crate::path::PathContext;
use crate::result::FinalReport;
use crate::spec::Spec;
use crate::value::Value;
use futures::channel::oneshot::Sender;
use futures::SinkExt;
use futures::{
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use lazy_static::__Deref;
use log::{info, trace};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tangram_finite::FiniteF64;

struct Context<'a> {
    spec: &'a Spec,
    _is_stochastic: bool,
    max_population_size: usize,
    individuals_evaled: BTreeMap<IndividualOrderingKey, Arc<Value>>,
    individuals_by_id: HashMap<usize, Arc<Value>>,
    max_num_eval: Option<usize>,
    eval_count: usize,
    completed_count: usize,
    rejected_count: usize,
    initial_value: Value,
    initial_value_job_sent: bool,
    crossover_params: CrossoverParams,
    mutation_params: MutationParams,
    crossover: Crossover<'a>,
    path: PathContext,
    rng: StdRng,
    next_id: usize,
    num_active_workers: usize,
}

impl<'a> Context<'a> {
    fn new(spec: &'a Spec, algo_config: AlgoConfig, max_num_eval: Option<usize>) -> Self {
        Self {
            initial_value: spec.initial_value(),
            spec,
            _is_stochastic: algo_config.is_stochastic,
            max_population_size: algo_config.max_population_size,
            crossover_params: algo_config.init_crossover_params,
            mutation_params: algo_config.init_mutation_params,
            individuals_evaled: BTreeMap::new(),
            individuals_by_id: HashMap::new(),
            max_num_eval,
            eval_count: 0,
            completed_count: 0,
            rejected_count: 0,
            initial_value_job_sent: false,
            crossover: Crossover::new(spec),
            path: PathContext::default(),
            rng: StdRng::seed_from_u64(0),
            next_id: 0,
            num_active_workers: 0,
        }
    }
}

#[derive(Ord, Eq, PartialEq, PartialOrd, Clone, Debug)]
struct IndividualOrderingKey {
    obj_func_val: FiniteF64,
    id: usize,
}

fn finitify_obj_func_val(obj_func_val: f64) -> Result<FiniteF64, Error> {
    FiniteF64::new(obj_func_val).map_err(|_| Error::ObjFuncValMustBeFinite)
}

pub async fn start_controller(
    spec: Spec,
    algo_config: AlgoConfig,
    mut recv: UnboundedReceiver<ControllerEvent>,
    mut report_sender: UnboundedSender<Report>,
    max_num_eval: Option<usize>,
) -> Result<FinalReport, Error> {
    let mut ctx = Context::new(&spec, algo_config, max_num_eval);

    info!("Start processing");

    let start_ts = Instant::now();

    while let Some(event) = recv.next().await {
        match event {
            WorkerTerminating => {
                ctx.on_worker_terminating();
                if ctx.num_active_workers == 0 {
                    break;
                }
            }
            WorkerReady { eval_job_sender } => ctx.on_worker_ready(eval_job_sender),
            IndividualEvalCompleted {
                obj_func_val,
                individual_id,
                next_eval_job_sender,
            } => {
                let individual = ctx.individuals_by_id.get(&individual_id).unwrap();

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
                    ctx.process_individual_eval(individual_id, obj_func_val, individual.clone());
                } else {
                    trace!("Individual rejected:\n{}", individual.to_json());
                    ctx.process_rejected_individual(individual_id);
                }

                ctx.on_worker_available(next_eval_job_sender)
            }
            TerminationCommand => break,
        }
    }

    info!("Processing completed");

    match ctx.individuals_evaled.into_iter().next() {
        Some((ordering_key, individual)) => Ok(FinalReport::new(
            ordering_key.obj_func_val.get(),
            individual.to_json(),
            ctx.completed_count,
            ctx.rejected_count,
            start_ts.elapsed(),
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

    fn on_worker_ready(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        self.num_active_workers += 1;
        self.on_worker_available(eval_job_sender)
    }

    fn on_worker_terminating(&mut self) {
        self.num_active_workers -= 1;
    }

    fn make_id(&mut self) -> usize {
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    fn create_and_send_next_eval_job(&mut self, eval_job_sender: Sender<IndividualEvalJob>) {
        let individual_id = self.make_id();
        let individual = if self.initial_value_job_sent {
            self.create_offspring(individual_id)
        } else {
            self.initial_value_job_sent = true;
            Arc::new(self.initial_value.clone())
        };

        self.individuals_by_id
            .insert(individual_id, individual.clone());

        let eval_job = IndividualEvalJob {
            individual,
            individual_id,
        };

        eval_job_sender.send(eval_job).ok();
    }

    fn create_offspring(&mut self, _individual_id: usize) -> Arc<Value> {
        let crossover_result = if self.individuals_evaled.is_empty() {
            self.initial_value.clone()
        } else {
            let individuals_ordered: Vec<&Value> = self
                .individuals_evaled
                .iter()
                .map(|ind| ind.1.deref())
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

        Arc::new(result)
    }

    fn process_individual_eval(
        &mut self,
        individual_id: usize,
        obj_func_val: FiniteF64,
        individual: Arc<Value>,
    ) {
        self.completed_count += 1;

        let ordering_key = IndividualOrderingKey {
            obj_func_val,
            id: individual_id,
        };

        let new_best = self
            .individuals_evaled
            .keys()
            .next()
            .map(|ordering_key| obj_func_val < ordering_key.obj_func_val)
            .unwrap_or(true);

        if new_best {
            info!("New best objective function value: {}", obj_func_val);
        }

        self.individuals_evaled.insert(ordering_key, individual);

        while self.individuals_evaled.len() > self.max_population_size {
            let last_ordering_key: IndividualOrderingKey =
                self.individuals_evaled.keys().next_back().unwrap().clone();
            self.individuals_evaled.remove(&last_ordering_key);
            self.individuals_by_id.remove(&last_ordering_key.id);
        }
    }

    fn process_rejected_individual(&mut self, _individual_id: usize) {
        self.rejected_count += 1;
    }
}
