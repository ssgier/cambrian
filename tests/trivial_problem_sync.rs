use cambrian::meta::AlgoConfigBuilder;
use cambrian::{self, meta, spec_util};
use cambrian::{sync_launch, termination::TerminationCriterion};
use float_cmp::approx_eq;
use serde::Deserialize;
use std::time::Duration;

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

    let algo_config = AlgoConfigBuilder::new().build().unwrap();

    let termination_criteria = vec![
        TerminationCriterion::TargetObjFuncVal(1e-6),
        TerminationCriterion::TerminateAfter(Duration::from_secs(1)),
    ];

    let result =
        sync_launch::launch(spec, obj_func, algo_config, termination_criteria, true).unwrap();

    let value = TestValue::deserialize(result.best_seen.value).unwrap();
    let obj_func_val = result.best_seen.obj_func_val;

    assert!(approx_eq!(f64, obj_func_val, 0.0, epsilon = 1e-2));
    assert!(approx_eq!(f64, value.x, 0.0, epsilon = 1e-2));
    assert!(approx_eq!(f64, value.y, 0.0, epsilon = 1e-2));
}
