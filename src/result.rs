use std::{fmt::Display, time::Duration};

#[derive(Debug)]
pub struct FinalReport {
    pub best_seen: BestSeen,
    pub num_obj_func_eval: usize,
    pub num_obj_func_eval_rejected: usize,
    pub processing_time: Duration,
}

impl FinalReport {
    pub fn new(
        obj_func_val: f64,
        value: serde_json::Value,
        num_obj_func_eval: usize,
        num_obj_func_eval_rejected: usize,
        processing_time: Duration,
    ) -> Self {
        Self {
            best_seen: BestSeen {
                obj_func_val,
                value,
            },
            num_obj_func_eval,
            num_obj_func_eval_rejected,
            processing_time,
        }
    }
}

#[derive(Debug)]
pub struct BestSeen {
    pub obj_func_val: f64,
    pub value: serde_json::Value,
}

impl Display for FinalReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Best seen objective function value: {}
Number of completed objective function evaluations: {}
Number of rejected objective function evaluations: {}
Processing time: {} seconds
        ",
            self.best_seen.obj_func_val,
            self.num_obj_func_eval,
            self.num_obj_func_eval_rejected,
            self.processing_time.as_secs_f64()
        )
    }
}
