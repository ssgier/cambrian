use cambrian::meta::AlgoConfigBuilder;
use cambrian::meta::{CrossoverParams, MutationParams};
use cambrian::{self, meta, spec_util};
use cambrian::{sync_launch, termination::TerminationCriterion};
use float_cmp::approx_eq;

fn extract_anon_map_size(value: &serde_json::Value) -> usize {
    let mapping = match value {
        serde_json::Value::Object(mapping) => mapping,
        _ => unreachable!(),
    };

    mapping.len()
}

#[test]
fn anon_map() {
    let spec_str = "
    type: anon map
    valueType:
        type: bool
    initSize: 0
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let target_size = 3usize;

    let obj_func = meta::make_obj_func(move |value| {
        let anon_map_size = extract_anon_map_size(&value);
        let result =
            f64::from(i32::try_from(anon_map_size).unwrap() - i32::try_from(target_size).unwrap())
                .abs();
        Some(result)
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

    let termination_criteria = vec![TerminationCriterion::NumObjFuncEval(100)];
    let result = sync_launch::launch(spec, obj_func, algo_config, termination_criteria).unwrap();

    let obj_func_val = result.best_seen.obj_func_val;
    let anon_map_size = extract_anon_map_size(&result.best_seen.value);
    assert!(approx_eq!(f64, obj_func_val, 0.0));
    assert_eq!(anon_map_size, target_size);
}
