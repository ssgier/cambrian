#[derive(Debug)]
pub struct FinalReport {
    pub best_seen: BestSeen,
}

impl FinalReport {
    pub fn from_best_seen(obj_func_val: f64, value: serde_json::Value) -> Self {
        Self {
            best_seen: BestSeen {
                obj_func_val,
                value,
            },
        }
    }
}

#[derive(Debug)]
pub struct BestSeen {
    pub obj_func_val: f64,
    pub value: serde_json::Value,
}
