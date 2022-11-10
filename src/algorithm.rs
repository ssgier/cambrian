pub(crate) use crate::crossover::Crossover;
use crate::meta::MetaParamsSource;
use crate::meta::MetaParamsWrapper;
use crate::meta_adapt;
use crate::mutation;
use crate::selection::{Selection, SelectionImpl};
use crate::value::Value;
use crate::{
    meta::{CrossoverParams, MutationParams},
    path::PathContext,
    spec::Spec,
};
use itertools::Itertools;
use log::{info, trace};
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Bernoulli, Distribution};
use std::collections::BTreeMap;
use tangram_finite::FiniteF64;

const STATIC_PARAMS: StaticParams = StaticParams {
    meta_params_prob_exploratory: 0.25,
    meta_params_select_pressure: 0.9,
    meta_params_prob_mutation: 0.5,
    prob_reeval: 0.5,
    min_pop_size_for_reeval: 20,
    max_pop_size: 1000,
};

struct StaticParams {
    meta_params_prob_exploratory: f64,
    meta_params_select_pressure: f64,
    meta_params_prob_mutation: f64,
    prob_reeval: f64,
    min_pop_size_for_reeval: usize,
    max_pop_size: usize,
}

pub struct AlgoContext {
    spec: Spec,
    individual_sample_size: usize,
    obj_func_val_quantile: f64,

    individuals: BTreeMap<OrderingKey, IndContext>,
    initial_value: Value,
    initial_value_used: bool,
    crossover: Crossover,
    path_ctx: PathContext,
    rng: StdRng,
    next_id: usize,
    meta_params_override: Option<(CrossoverParams, MutationParams)>,
    static_params: StaticParams,
}

impl AlgoContext {
    pub fn new(
        spec: Spec,
        individual_sample_size: usize,
        obj_func_val_quantile: f64,
        meta_params_override: Option<(CrossoverParams, MutationParams)>,
        explicit_init_value: Option<Value>,
    ) -> Self {
        Self::new_impl(
            spec,
            individual_sample_size,
            obj_func_val_quantile,
            meta_params_override,
            explicit_init_value,
            STATIC_PARAMS,
        )
    }

    fn new_impl(
        spec: Spec,
        individual_sample_size: usize,
        obj_func_val_quantile: f64,
        meta_params_override: Option<(CrossoverParams, MutationParams)>,
        explicit_init_value: Option<Value>,
        static_params: StaticParams,
    ) -> Self {
        Self {
            initial_value: explicit_init_value.unwrap_or_else(|| spec.initial_value()),
            spec,
            individual_sample_size,
            obj_func_val_quantile,
            individuals: BTreeMap::default(),
            initial_value_used: false,
            crossover: Crossover::new(),
            path_ctx: PathContext::default(),
            rng: StdRng::seed_from_u64(0),
            next_id: 0,
            static_params,
            meta_params_override,
        }
    }
}

#[derive(Ord, Eq, PartialEq, PartialOrd, Clone, Debug)]
struct OrderingKey {
    obj_func_val: FiniteF64,
    id: usize,
}

impl OrderingKey {
    fn new(id: usize, obj_func_val: FiniteF64) -> Self {
        Self { obj_func_val, id }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum IndState {
    PendingEval(Vec<FiniteF64>),
    Ready(Vec<FiniteF64>),
    Final(FiniteF64),
}

#[derive(Debug)]
pub struct IndContext {
    pub id: usize,
    pub value: Value,
    pub meta_params_used: Option<MetaParamsWrapper>,
    state: IndState,
}

impl IndContext {
    fn new(id: usize, value: Value, meta_params_used: Option<MetaParamsWrapper>) -> Self {
        Self {
            id,
            value,
            meta_params_used,
            state: IndState::PendingEval(Vec::default()),
        }
    }
}

fn summary_obj_func_val(obj_func_vals: &[FiniteF64], quantile: f64) -> FiniteF64 {
    let n = obj_func_vals.len().to_f64().unwrap();
    let loc_target = (n - 1.0) * quantile;
    let left_idx_f64 = loc_target.floor();
    let left_idx = left_idx_f64.to_usize().unwrap();
    let right_idx = loc_target.ceil().to_usize().unwrap();
    let left_val = obj_func_vals[left_idx].get();
    let right_val = obj_func_vals[right_idx].get();
    let result = left_val + (loc_target - left_idx_f64) * (right_val - left_val);
    FiniteF64::new(result).unwrap()
}

impl AlgoContext {
    fn make_id(&mut self) -> usize {
        let result = self.next_id;
        self.next_id += 1;
        result
    }

    fn extract_best_ready(&mut self) -> Option<IndContext> {
        let key = self
            .individuals
            .iter()
            .filter(|entry| matches!(entry.1.state, IndState::Ready(_)))
            .map(|entry| entry.0)
            .next();

        if let Some(key) = key {
            Some(self.individuals.remove(&key.clone()).unwrap())
        } else {
            None
        }
    }

    fn try_reeval(&mut self) -> bool {
        self.individual_sample_size > 1
            && self.individuals.len() >= self.static_params.min_pop_size_for_reeval
            && Bernoulli::new(self.static_params.prob_reeval)
                .unwrap()
                .sample(&mut self.rng)
    }

    pub fn next_individual(&mut self) -> IndContext {
        if self.try_reeval() {
            if let Some(mut ind_ctx) = self.extract_best_ready() {
                let state = if let IndState::Ready(obj_func_vals) = ind_ctx.state {
                    IndState::PendingEval(obj_func_vals)
                } else {
                    unreachable!();
                };

                ind_ctx.state = state;
                info!(
                    "Individual {}: selected individual for re-evaluation",
                    ind_ctx.id
                );
                return ind_ctx;
            }
        }

        let (value, meta_params_used) = if !self.initial_value_used {
            self.initial_value_used = true;
            (self.initial_value.clone(), None)
        } else {
            let (value, meta_params_wrapper) = self.create_offspring();

            (value, Some(meta_params_wrapper))
        };

        let id = self.make_id();

        info!("Individual {}: Created", id);

        IndContext::new(id, value, meta_params_used)
    }

    fn create_offspring(&mut self) -> (Value, MetaParamsWrapper) {
        let meta_params_wrapper = self.next_meta_params();

        let individuals_ordered: Vec<&Value> =
            self.individuals.values().map(|ctx| &ctx.value).collect();
        let crossover_result = if individuals_ordered.is_empty() {
            self.initial_value.clone()
        } else {
            self.crossover.crossover(
                &self.spec,
                &individuals_ordered,
                &meta_params_wrapper.crossover_params,
                &mut self.path_ctx,
                &mut self.rng,
            )
        };

        let result = mutation::mutate(
            &self.spec,
            &crossover_result,
            &meta_params_wrapper.mutation_params,
            &mut self.path_ctx,
            &mut self.rng,
        );

        trace!(
            "Offspring created:\ncrossover result:\n{}\nmutation result:\n{}",
            crossover_result.to_json(),
            result.to_json()
        );

        (result, meta_params_wrapper)
    }

    fn next_meta_params(&mut self) -> MetaParamsWrapper {
        if let Some(meta_params_override) = &self.meta_params_override {
            return wrap(meta_params_override.clone(), MetaParamsSource::Override);
        }

        if Bernoulli::new(self.static_params.meta_params_prob_exploratory)
            .unwrap()
            .sample(&mut self.rng)
        {
            wrap(
                meta_adapt::create_exploratory(&mut self.rng),
                MetaParamsSource::Exploratory,
            )
        } else {
            let meta_params_ordered = self
                .individuals
                .values()
                .filter_map(|ctx| ctx.meta_params_used.as_ref())
                .collect_vec();

            if meta_params_ordered.is_empty() {
                wrap(
                    meta_adapt::create_exploratory(&mut self.rng),
                    MetaParamsSource::Exploratory,
                )
            } else {
                let selected = SelectionImpl::new()
                    .select_ref(
                        &meta_params_ordered,
                        self.static_params.meta_params_select_pressure,
                        &mut self.rng,
                    )
                    .clone();

                if Bernoulli::new(self.static_params.meta_params_prob_mutation)
                    .unwrap()
                    .sample(&mut self.rng)
                {
                    wrap(
                        meta_adapt::mutate(
                            selected.crossover_params,
                            selected.mutation_params,
                            &mut self.rng,
                        ),
                        MetaParamsSource::SelectedAndMutated,
                    )
                } else {
                    wrap(
                        (selected.crossover_params, selected.mutation_params),
                        MetaParamsSource::Selected,
                    )
                }
            }
        }
    }

    fn summary_obj_func_val(&self, ind_state: &IndState) -> FiniteF64 {
        match *ind_state {
            IndState::PendingEval(ref obj_func_vals) | IndState::Ready(ref obj_func_vals) => {
                summary_obj_func_val(obj_func_vals, self.obj_func_val_quantile)
            }
            IndState::Final(obj_func_val) => obj_func_val,
        }
    }

    fn transition_state(&self, state: IndState, obj_func_val: FiniteF64, id: usize) -> IndState {
        if let IndState::PendingEval(mut obj_func_vals) = state {
            obj_func_vals.push(obj_func_val);

            if obj_func_vals.len() == self.individual_sample_size {
                let summary_obj_func_val =
                    summary_obj_func_val(&obj_func_vals, self.obj_func_val_quantile);

                info!(
                    "Individual {}: completed sample, final objective function value: {}",
                    id, summary_obj_func_val
                );

                IndState::Final(summary_obj_func_val)
            } else {
                IndState::Ready(obj_func_vals)
            }
        } else {
            unreachable!()
        }
    }

    fn log_top_obj_func_vals(&self) {
        if self.individuals.len() < 2 {
            return;
        }

        let num_to_take = 10.min(self.individuals.len());

        let summary_obj_func_vals: Vec<f64> = self
            .individuals
            .values()
            .take(num_to_take)
            .map(|ctx| match ctx.state {
                IndState::PendingEval(ref obj_func_vals) | IndState::Ready(ref obj_func_vals) => {
                    summary_obj_func_val(obj_func_vals, self.obj_func_val_quantile).get()
                }
                IndState::Final(obj_func_val) => obj_func_val.get(),
            })
            .collect();

        let mean: f64 = summary_obj_func_vals.iter().sum::<f64>() / num_to_take.to_f64().unwrap();

        info!(
            "Top {} objective function values with mean {}: {:?}",
            num_to_take, mean, summary_obj_func_vals
        );
    }

    pub fn process_individual_eval(
        &mut self,
        mut ind_ctx: IndContext,
        obj_func_val: Option<FiniteF64>,
    ) {
        if let Some(obj_func_val) = obj_func_val {
            info!(
                "Individual {}: received objective function value: {}",
                ind_ctx.id,
                obj_func_val.get()
            );

            ind_ctx.state = self.transition_state(ind_ctx.state, obj_func_val, ind_ctx.id);

            let ordering_key =
                OrderingKey::new(ind_ctx.id, self.summary_obj_func_val(&ind_ctx.state));
            self.individuals.insert(ordering_key, ind_ctx);

            while self.individuals.len() > self.static_params.max_pop_size {
                let key_to_remove = self.individuals.iter().next_back().unwrap().0.clone();
                self.individuals.remove(&key_to_remove);
            }

            self.log_top_obj_func_vals();
        } else {
            info!("Individual {}: value rejected", ind_ctx.id);
        }
    }

    pub fn best_seen_final(&self) -> Option<(FiniteF64, &Value)> {
        self.individuals.values().find_map(|ctx| {
            if let IndState::Final(obj_func_val) = ctx.state {
                Some((obj_func_val, &ctx.value))
            } else {
                None
            }
        })
    }
}

fn wrap(
    unwrapped: (CrossoverParams, MutationParams),
    source: MetaParamsSource,
) -> MetaParamsWrapper {
    let (crossover_params, mutation_params) = unwrapped;

    MetaParamsWrapper {
        source,
        crossover_params,
        mutation_params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec;
    use crate::value;
    use float_cmp::assert_approx_eq;

    const NEVER_CROSSOVER: CrossoverParams = CrossoverParams {
        crossover_prob: 0.0,
        selection_pressure: 1.0,
    };

    const ALWAYS_MUTATE: MutationParams = MutationParams {
        mutation_prob: 1.0,
        mutation_scale: 1.0,
    };

    const TRIVIAL_SPEC: Spec = spec::Spec(spec::Node::Bool { init: true });

    fn make_sut() -> AlgoContext {
        AlgoContext::new(
            TRIVIAL_SPEC,
            1,
            1.0,
            Some((NEVER_CROSSOVER, ALWAYS_MUTATE)),
            None,
        )
    }

    fn make_result(id: usize, value: bool, obj_func_val: f64) -> (IndContext, Option<FiniteF64>) {
        (
            IndContext::new(
                id,
                Value(value::Node::Bool(value)),
                Some(MetaParamsWrapper::new(
                    MetaParamsSource::Override,
                    NEVER_CROSSOVER,
                    ALWAYS_MUTATE,
                )),
            ),
            Some(FiniteF64::new(obj_func_val).unwrap()),
        )
    }

    #[test]
    fn initial_guess_then_mutation() {
        let mut sut = make_sut();

        assert_eq!(sut.next_individual().value.0, value::Node::Bool(true));
        for _ in 0..2 {
            assert_eq!(sut.next_individual().value.0, value::Node::Bool(false));
        }

        assert_eq!(sut.best_seen_final(), None);
    }

    #[test]
    fn initial_guess_ignored_after_first_evaluation() {
        let mut sut = make_sut();

        for _ in 0..2 {
            sut.next_individual();
        }

        let (ind_ctx, obj_func_val) = make_result(0, false, 0.1);

        sut.process_individual_eval(ind_ctx, obj_func_val);

        for _ in 0..2 {
            assert_eq!(sut.next_individual().value.0, value::Node::Bool(true));
        }

        if let Some((obj_func_val, value)) = sut.best_seen_final() {
            assert_eq!(obj_func_val.get(), 0.1);
            assert_eq!(*value, Value(value::Node::Bool(false)));
        } else {
            panic!()
        }
    }

    #[test]
    fn best_seen_overtaken() {
        let mut sut = make_sut();
        sut.next_individual();
        assert_eq!(sut.best_seen_final(), None);

        let (ind_ctx, obj_func_val) = make_result(0, true, 0.2);
        sut.process_individual_eval(ind_ctx, obj_func_val);

        assert_eq!(
            sut.best_seen_final()
                .map(|(obj_func_val, _)| obj_func_val.get()),
            Some(0.2)
        );
        assert_eq!(sut.next_individual().value.0, value::Node::Bool(false));

        let (ind_ctx, obj_func_val) = make_result(1, false, 0.1);
        sut.process_individual_eval(ind_ctx, obj_func_val);

        assert_eq!(sut.next_individual().value.0, value::Node::Bool(true));

        let (obj_func_val, value) = sut.best_seen_final().unwrap();
        assert_eq!(obj_func_val.get(), 0.1);

        assert_eq!(*value, value::Value(value::Node::Bool(false)));
    }

    #[test]
    fn max_population_size() {
        let static_params = StaticParams {
            max_pop_size: 1,
            ..STATIC_PARAMS
        };

        let mut sut = AlgoContext::new_impl(
            TRIVIAL_SPEC,
            1,
            1.0,
            Some((NEVER_CROSSOVER, ALWAYS_MUTATE)),
            None,
            static_params,
        );

        sut.next_individual();

        let (ind_ctx, obj_func_val) = make_result(0, true, 0.3);
        sut.process_individual_eval(ind_ctx, obj_func_val);
        assert_eq!(sut.individuals.len(), 1);

        let (ind_ctx, obj_func_val) = make_result(1, true, 0.2);
        sut.process_individual_eval(ind_ctx, obj_func_val);
        assert_eq!(sut.individuals.len(), 1);
        assert_eq!(sut.individuals.values().next().unwrap().id, 1); // the individual 0 was evicted
    }

    #[test]
    fn reeval() {
        let sample_size = 2;

        let static_params = StaticParams {
            min_pop_size_for_reeval: 2,
            prob_reeval: 1.0,
            ..STATIC_PARAMS
        };

        let mut sut = AlgoContext::new_impl(
            TRIVIAL_SPEC,
            sample_size,
            1.0,
            Some((NEVER_CROSSOVER, ALWAYS_MUTATE)),
            None,
            static_params,
        );

        sut.next_individual();

        let (ind_ctx, obj_func_val) = make_result(0, true, 0.2);
        sut.process_individual_eval(ind_ctx, obj_func_val);
        let top_individual = sut.individuals.values().next().unwrap();
        assert!(matches!(
            top_individual,
            IndContext {
                id: 0,
                value: Value(value::Node::Bool(true)),
                state: IndState::Ready(obj_func_vals),
                ..
            } if *obj_func_vals == vec![FiniteF64::new(0.2).unwrap()]
        ));

        // mutated offspring created
        let next_individual = sut.next_individual();
        assert_eq!(next_individual.value.0, value::Node::Bool(false));

        sut.process_individual_eval(next_individual, Some(FiniteF64::new(0.3).unwrap()));
        assert_eq!(sut.individuals.len(), 2);

        // picking the first individual for reevaluation
        let next_individual = sut.next_individual();
        assert_eq!(sut.individuals.len(), 1);
        assert!(matches!(
            next_individual,
            IndContext {
                id: 0,
                value: Value(value::Node::Bool(true)),
                state: IndState::PendingEval(ref obj_func_vals),
                ..
            } if *obj_func_vals == vec![FiniteF64::new(0.2).unwrap()]
        ));

        // reevaluation result
        sut.process_individual_eval(next_individual, Some(FiniteF64::new(0.1).unwrap()));

        // context state transitioned to final after two evaluations
        let top_individual = sut.individuals.values().next().unwrap();
        assert!(matches!(
            *top_individual,
            IndContext {
                id: 0,
                value: Value(value::Node::Bool(true)),
                state: IndState::Final(obj_func_vals),
                ..
            } if obj_func_vals == FiniteF64::new(0.1).unwrap()
        ));
        assert_eq!(sut.individuals.len(), 2);

        // picking the second individual for reevaluation
        let ind_1_for_reeval = sut.next_individual();
        assert_eq!(sut.individuals.len(), 1);
        assert!(matches!(
            ind_1_for_reeval,
            IndContext {
                id: 1,
                value: Value(value::Node::Bool(false)),
                state: IndState::PendingEval(ref obj_func_vals),
                ..
            } if *obj_func_vals == vec![FiniteF64::new(0.3).unwrap()]
        ));

        // mutated offspring created based on first individual
        let next_individual = sut.next_individual();
        assert_eq!(sut.individuals.len(), 1);
        assert!(matches!(
            next_individual,
            IndContext {
                id: 2,
                value: Value(value::Node::Bool(false)),
                state: IndState::PendingEval(ref obj_func_vals),
                ..
            } if *obj_func_vals == vec![]
        ));

        // comes back first, better than second, but worse than first
        sut.process_individual_eval(next_individual, Some(FiniteF64::new(0.25).unwrap()));
        assert_eq!(sut.individuals.len(), 2);

        // second individual comes back after final evaluation, ranks worst
        sut.process_individual_eval(ind_1_for_reeval, Some(FiniteF64::new(0.4).unwrap()));
        assert_eq!(sut.individuals.len(), 3);

        let ind_ids_in_ranking_order: Vec<usize> =
            sut.individuals.values().map(|ctx| ctx.id).collect();
        assert_eq!(ind_ids_in_ranking_order, vec![0, 2, 1]);
    }

    #[test]
    fn quantile_single_value() {
        let values = vec![FiniteF64::new(1.0).unwrap()];
        assert_eq!(summary_obj_func_val(&values, 0.0).get(), 1.0);
        assert_eq!(summary_obj_func_val(&values, 0.5).get(), 1.0);
        assert_eq!(summary_obj_func_val(&values, 1.0).get(), 1.0);
    }

    #[test]
    fn quantile_uneven_length() {
        let values = vec![0.1, 1.1, 2.1, 3.1, 4.1];
        let values: Vec<FiniteF64> = values
            .into_iter()
            .map(|val| FiniteF64::new(val).unwrap())
            .collect();

        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.0).get(), 0.1);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.25).get(), 1.1);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.375).get(), 1.6);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.5).get(), 2.1);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 1.0).get(), 4.1);
    }

    #[test]
    fn quantile_even_length() {
        let values = vec![0.1, 1.1, 2.1, 3.1];
        let values: Vec<FiniteF64> = values
            .into_iter()
            .map(|val| FiniteF64::new(val).unwrap())
            .collect();

        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.0).get(), 0.1);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 0.5).get(), 1.6);
        assert_approx_eq!(f64, summary_obj_func_val(&values, 1.0).get(), 3.1);
    }
}
