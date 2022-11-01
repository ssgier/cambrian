use std::time::Duration;

use cambrian::meta::AlgoConfigBuilder;
use cambrian::meta::{CrossoverParams, MutationParams};
use cambrian::{self, meta, spec_util};
use cambrian::{sync_launch, termination::TerminationCriterion};
use float_cmp::approx_eq;
use std::thread;

#[test]
fn terminate_after() {
    let spec_str = "
    type: bool
    init: true
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let obj_func = meta::make_obj_func(|_| {
        thread::sleep(Duration::from_millis(100));
        Some(0.1)
    });

    let init_crossover_params = CrossoverParams {
        crossover_prob: 0.75,
        selection_pressure: 0.5,
    };

    let init_mutation_params = MutationParams {
        mutation_prob: 0.8,
        mutation_scale: 1.0,
    };

    let algo_config = AlgoConfigBuilder::new()
        .init_crossover_params(init_crossover_params)
        .init_mutation_params(init_mutation_params)
        .build();

    let termination_criteria = vec![TerminationCriterion::TerminateAfter(Duration::from_millis(
        150,
    ))];

    let result = sync_launch::launch(spec, obj_func, algo_config, termination_criteria, true);
    assert!(result.is_ok());
    let result = result.unwrap();

    let obj_func_val = result.best_seen.obj_func_val;

    println!("{}", result);
    assert_eq!(result.num_obj_func_eval_completed, 1);
    assert_eq!(result.num_obj_func_eval_rejected, 1);
    assert!(approx_eq!(
        f64,
        result.processing_time.as_secs_f64(),
        0.15,
        epsilon = 25e-3
    ));
    assert!(approx_eq!(f64, obj_func_val, 0.1, epsilon = 1e-12));
}
