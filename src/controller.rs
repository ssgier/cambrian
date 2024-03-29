use crate::algorithm::AlgoContext;
use crate::algorithm::IndContext;
use crate::detailed_report::DetailedReportItem;
use crate::error::Error;
use crate::spec::Spec;
use crate::value_util;
use crate::{
    meta::{AlgoConfig, AsyncObjectiveFunction},
    result::FinalReport,
};
use futures::channel::mpsc::Sender;
use futures::channel::oneshot;
use futures::stream::FuturesUnordered;
use futures::SinkExt;
use futures::TryStreamExt;
use log::info;
use std::time::{Duration, Instant};
use tangram_finite::FiniteF64;

#[allow(clippy::too_many_arguments)]
pub async fn start_controller<F: AsyncObjectiveFunction>(
    algo_config: AlgoConfig,
    spec: Spec,
    obj_func: F,
    mut in_abort_signal_recv: oneshot::Receiver<()>,
    mut detailed_report_sender: Sender<DetailedReportItem>,
    max_num_eval: Option<usize>,
    target_obj_func_val: Option<f64>,
    explicit_init_value_json: Option<serde_json::Value>,
) -> Result<FinalReport, Error> {
    let start_ts = Instant::now();

    info!("Start processing");

    let mut seed_mgr = SeedManager::new();

    let (abort_signal_sender, out_abort_signal_recv) = async_broadcast::broadcast::<()>(1);
    let mut count_accepted = 0usize;
    let mut count_rejected = 0usize;

    let explicit_init_value = explicit_init_value_json
        .map(|json_val| value_util::from_json_value(&json_val, &spec))
        .transpose()?;

    let mut algo_ctx = AlgoContext::new(
        spec,
        algo_config.individual_sample_size,
        None,
        explicit_init_value,
    );

    let mut evaled_individuals = FuturesUnordered::new();
    let mut abort_signal_received = false;
    let mut pushed_for_eval_count = 0;

    let initial_num_individuals = if let Some(max_num_eval) = max_num_eval {
        algo_config.num_concurrent.min(max_num_eval)
    } else {
        algo_config.num_concurrent
    };

    for _ in 0..initial_num_individuals {
        evaled_individuals.push(evaluate_individual(
            algo_ctx.next_individual(),
            &obj_func,
            out_abort_signal_recv.clone(),
            seed_mgr.next_seed(),
        ));
    }

    pushed_for_eval_count += initial_num_individuals;

    let mut error_recording = None;

    loop {
        tokio::select! {
            evaled_individual = &mut evaled_individuals.try_next() => {
                match evaled_individual {

                    Ok(None) => break,
                    Ok(Some(evaled_individual)) => {

                        let detailed_report_item = DetailedReportItem {
                            individual_id: evaled_individual.ind_ctx.id,
                            eval_time: evaled_individual.eval_time,
                            meta_params_used: evaled_individual.ind_ctx.meta_params_used.clone(),
                            input_val: evaled_individual.ind_ctx.value.to_json(),
                            seed: evaled_individual.seed,
                            obj_func_val: evaled_individual.obj_func_val.map(FiniteF64::get),
                        };

                        detailed_report_sender.send(detailed_report_item).await.map_err(|_err| Error::ClientHungUp)?;

                        if evaled_individual.obj_func_val.is_some() {
                            count_accepted += 1;
                        } else {
                            count_rejected += 1;
                        }
                        algo_ctx.process_individual_eval(evaled_individual.ind_ctx, evaled_individual.obj_func_val);


                        if let (Some(target_obj_func_val), Some(best_seen_final)) =
                        (target_obj_func_val, algo_ctx.best_seen_final()) {
                            if best_seen_final.0.get() <= target_obj_func_val {
                                break;
                            }
                        }

                        let (max_num_eval_pushed, max_num_eval_completed) = if let Some(max_num_eval) = max_num_eval {
                            (pushed_for_eval_count >= max_num_eval, count_accepted + count_rejected >= max_num_eval)
                        } else {
                            (false, false)
                        };

                        if max_num_eval_completed {
                            break;
                        } else if !max_num_eval_pushed && !abort_signal_received {
                            let new_individual = algo_ctx.next_individual();
                            let eval_future = evaluate_individual(new_individual, &obj_func, out_abort_signal_recv.clone(),
                                seed_mgr.next_seed());
                            evaled_individuals.push(eval_future);
                            pushed_for_eval_count += 1;
                        }
                    }
                    Err(error) => {
                        if !abort_signal_received {
                            abort_signal_received = true;
                            abort_signal_sender.broadcast(()).await.unwrap();
                            error_recording = Some(error);
                        }
                    }
                }
            }
            _ = &mut in_abort_signal_recv => {
                if !abort_signal_received {
                    abort_signal_received = true;
                    abort_signal_sender.broadcast(()).await.unwrap();
                }
            }
        }
    }

    if let Some(error) = error_recording {
        return Err(error);
    }

    info!("Processing completed");

    match algo_ctx.best_seen_final() {
        Some(best_seen) => Ok(FinalReport::new(
            best_seen.0.get(),
            best_seen.1.to_json(),
            count_accepted,
            count_rejected,
            start_ts.elapsed(),
        )),
        None => Err(Error::NoIndividuals),
    }
}

struct EvaluatedIndividual {
    obj_func_val: Option<FiniteF64>,
    ind_ctx: IndContext,
    eval_time: Duration,
    seed: u64,
}

async fn evaluate_individual<F: AsyncObjectiveFunction>(
    individual: IndContext,
    obj_func: &F,
    abort_signal_recv: async_broadcast::Receiver<()>,
    seed: u64,
) -> Result<EvaluatedIndividual, Error> {
    let start_time = Instant::now();

    let eval_result = obj_func
        .evaluate(
            individual.value.to_json(),
            abort_signal_recv,
            seed,
            individual.id,
        )
        .await?;

    let eval_time = start_time.elapsed();

    let finitified_result = eval_result
        .map(FiniteF64::new)
        .transpose()
        .map_err(|_| Error::ObjFuncValMustBeFinite)?;

    Ok(EvaluatedIndividual {
        obj_func_val: finitified_result,
        ind_ctx: individual,
        eval_time,
        seed,
    })
}

struct SeedManager {
    next_seed: u64,
}

impl SeedManager {
    fn new() -> Self {
        Self { next_seed: 0 }
    }

    fn next_seed(&mut self) -> u64 {
        let result = self.next_seed;
        self.next_seed += 1;
        result
    }
}
