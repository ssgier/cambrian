use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Command {
    Terminate,
}

#[derive(Serialize, Deserialize)]
pub enum Report {
    IndividualEvalCompleted {
        obj_func_val: Option<f64>,
        individual: serde_json::Value,
    },
}
