use cambrian::error::Error;
use cambrian::meta::AlgoConfigBuilder;
use cambrian::result::FinalReport;
use cambrian::{self, meta, spec_util};
use cambrian::{sync_launch, termination::TerminationCriterion};
use float_cmp::{approx_eq, assert_approx_eq};
use std::thread;
use std::time::Duration;

fn run(
    termination_criteria: Vec<TerminationCriterion>,
    sleep_duration: Duration,
) -> Result<FinalReport, Error> {
    let spec_str = "
    type: bool
    init: true
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let obj_func = meta::make_obj_func(move |_| {
        thread::sleep(sleep_duration);
        Some(0.1)
    });

    let algo_config = AlgoConfigBuilder::new().build().unwrap();

    sync_launch::launch(spec, obj_func, algo_config, termination_criteria, true)
}

#[test]
fn terminate_after() {
    let termination_criteria = vec![TerminationCriterion::TerminateAfter(Duration::from_millis(
        150,
    ))];

    let result = run(termination_criteria, Duration::from_millis(100)).unwrap();
    let obj_func_val = result.best_seen.obj_func_val;

    assert_eq!(result.num_obj_func_eval_completed, 1);
    assert_eq!(result.num_obj_func_eval_rejected, 1);
    assert!(approx_eq!(
        f64,
        result.processing_time.as_secs_f64(),
        0.15,
        epsilon = 25e-3
    ));
    assert_approx_eq!(f64, obj_func_val, 0.1);
}

#[test]
fn max_num_obj_func_eval() {
    let termination_criteria = vec![TerminationCriterion::NumObjFuncEval(11)];
    let result = run(termination_criteria, Duration::ZERO).unwrap();
    let obj_func_val = result.best_seen.obj_func_val;
    assert_eq!(result.num_obj_func_eval_completed, 11);
    assert_approx_eq!(f64, obj_func_val, 0.1);
}

#[test]
fn target_obj_func_val() {
    let max_num_obj_func_eval = 2;
    let not_reached_target = 0.09;
    let termination_criteria = vec![
        TerminationCriterion::TargetObjFuncVal(not_reached_target),
        TerminationCriterion::NumObjFuncEval(max_num_obj_func_eval),
    ];
    let result = run(termination_criteria, Duration::ZERO).unwrap();
    assert_eq!(result.num_obj_func_eval_completed, max_num_obj_func_eval);

    let reached_target = 0.1;
    let termination_criteria = vec![
        TerminationCriterion::TargetObjFuncVal(reached_target),
        TerminationCriterion::NumObjFuncEval(max_num_obj_func_eval),
    ];
    let result = run(termination_criteria, Duration::ZERO);
    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.num_obj_func_eval_completed, 1);
}
