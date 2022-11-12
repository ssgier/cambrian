use cambrian::meta::{self, AlgoConfigBuilder};
use cambrian::spec_util;
use cambrian::{sync_launch, termination::TerminationCriterion};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// change if checksum is expected to change
const CHECKSUM: u64 = 15057406331161146692;

fn compute_hash(value: &serde_json::Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    let string_value = value.to_string();
    string_value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn checksum() {
    let spec_str = "
    foo:
      type: bool
      init: false
    bar:
      type: anon map
      initSize: 1
      minSize: 1
      maxSize: 2
      valueType:
        type: optional
        initPresent: true
        valueType:
          type: variant
          init: foo
          foo:
            type: enum
            values:
              - foo
              - bar
            init: foo
          bar:
            type: anon map
            initSize: 1
            valueType:
              x:
                type: real
                init: 1.0
                scale: 1.0
              y:
                type: int
                init: 1
                scale: 10
              z:
                type: bool
                init: false
          baz:
            type: const
    ";

    let spec = spec_util::from_yaml_str(spec_str).unwrap();

    let obj_func = meta::make_obj_func(|value| {
        let json_str = value.to_string();
        Some(-(json_str.len() as f64))
    });

    let algo_config = AlgoConfigBuilder::new().build().unwrap();

    let termination_criteria = vec![TerminationCriterion::NumObjFuncEval(1000)];

    let report = sync_launch::launch(
        spec,
        obj_func,
        algo_config,
        termination_criteria,
        None,
        true,
        None,
    )
    .unwrap();

    assert_eq!(compute_hash(&report.best_seen.value), CHECKSUM);
}
