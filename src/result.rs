pub struct FinalReport {
    _best_seen: BestSeen,
}

impl FinalReport {
    pub fn from_best_seen(obj_func_val: f64, value: serde_json::Value) -> Self {
        Self {
            _best_seen: BestSeen {
                _obj_func_val: obj_func_val,
                _value: value,
            },
        }
    }
}

pub struct BestSeen {
    _obj_func_val: f64,
    _value: serde_json::Value,
}
