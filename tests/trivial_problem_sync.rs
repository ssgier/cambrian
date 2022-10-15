use cambrian::{self, meta, spec_util};
use cambrian::{
    meta::{AlgoParams, CrossoverParams, MutationParams},
    sync_launch,
    termination::TerminationCriterion,
};
use float_cmp::approx_eq;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct TestValue {
    x: f64,
    y: f64,
}

#[test]
fn trivial_problem_sync() {
    let spec_str = "
    x:
        type: real
        init: 1.0
        scale: 0.1
    y:
        type: real
        init: 1.0
        scale: 0.1
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let obj_func = meta::make_obj_func(|value| {
        let value: TestValue = TestValue::deserialize(value).unwrap();
        let x = value.x;
        let y = value.y;
        Some(x * x + y * y)
    });

    let algo_params = AlgoParams {
        is_stochastic: false,
        num_concurrent: 1,
    };

    let init_crossover_params = CrossoverParams {
        crossover_prob: 0.75,
        selection_pressure: 0.5,
    };

    let init_mutation_params = MutationParams {
        mutation_prob: 0.8,
        mutation_scale: 1.0,
    };

    let termination_criteria = vec![TerminationCriterion::NumObjFuncEval(100)];
    let result = sync_launch::launch(
        spec,
        obj_func,
        algo_params,
        init_crossover_params,
        init_mutation_params,
        termination_criteria,
    )
    .unwrap();

    let value = TestValue::deserialize(result.best_seen.value).unwrap();
    let obj_func_val = result.best_seen.obj_func_val;

    assert!(approx_eq!(f64, obj_func_val, 0.0, epsilon = 1e-2));
    assert!(approx_eq!(f64, value.x, 0.0, epsilon = 1e-2));
    assert!(approx_eq!(f64, value.y, 0.0, epsilon = 1e-2));
}
