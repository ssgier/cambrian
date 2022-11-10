use cambrian::error::Error;
use cambrian::meta::AlgoConfigBuilder;
use cambrian::process::ObjFuncProcessDef;
use cambrian::result::FinalReport;
use cambrian::{self, spec_util};
use cambrian::{sync_launch, termination::TerminationCriterion};
use float_cmp::{approx_eq, assert_approx_eq};
use std::time::Duration;

fn run(
    script_name: &str,
    kill_obj_func_after: Duration,
    terminate_after: Duration,
) -> Result<FinalReport, Error> {
    let spec_str = "
    type: bool
    init: true
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let program_path = format!("{}/scripts/{}", env!("CARGO_MANIFEST_DIR"), script_name);

    let obj_func = ObjFuncProcessDef::new(program_path.into(), vec![], Some(kill_obj_func_after));

    let algo_config = AlgoConfigBuilder::new().build().unwrap();

    let termination_criteria = vec![TerminationCriterion::TerminateAfter(terminate_after)];
    sync_launch::launch_with_async_obj_func(
        spec,
        obj_func,
        algo_config,
        termination_criteria,
        None,
        false,
        None,
    )
}

#[test]
#[cfg(target_os = "linux")]
fn rejected_value() {
    let script_name = "mock_obj_func_reject.sh";
    let kill_after = Duration::from_millis(200);
    let result = run(script_name, kill_after, Duration::from_millis(1000));
    assert!(matches!(result.unwrap_err(), Error::NoIndividuals));
}

#[test]
#[cfg(target_os = "linux")]
fn obj_func_process_error_exit() {
    let script_name = "mock_obj_func_error.sh";
    let kill_after = Duration::from_millis(200);
    let result = run(script_name, kill_after, Duration::from_millis(1000));
    assert!(matches!(result.unwrap_err(), Error::ObjFuncProcFailed(_)));
}

#[test]
#[cfg(target_os = "linux")]
fn kill_after_timeout() {
    let script_name = "mock_obj_func_sleep_250.sh";
    let kill_after = Duration::from_millis(200);
    let result = run(script_name, kill_after, Duration::from_millis(1000));
    assert!(result.is_err());
}

#[test]
#[cfg(target_os = "linux")]
fn completed_before_timeout() {
    let script_name = "mock_obj_func_sleep_250.sh";
    let kill_obj_func_after = Duration::from_millis(400);
    let terminate_after = Duration::from_millis(700);
    let result = run(script_name, kill_obj_func_after, terminate_after).unwrap();
    let obj_func_val = result.best_seen.obj_func_val;

    assert_eq!(result.num_obj_func_eval_completed, 2);
    assert_eq!(result.num_obj_func_eval_rejected, 1);
    assert!(approx_eq!(
        f64,
        result.processing_time.as_secs_f64(),
        0.7,
        epsilon = 45e-3
    ));
    assert_approx_eq!(f64, obj_func_val, 0.1);
}
