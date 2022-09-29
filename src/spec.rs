use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spec(pub Node);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Node {
    Real {
        optional: bool,
        init: f64,
        scale: f64,
        min: Option<f64>,
        max: Option<f64>,
        prob_dist: RealProbDist,
    },
    Int {
        optional: bool,
        init: i64,
        scale: f64,
        min: Option<i64>,
        max: Option<i64>,
        prob_dist: IntProbDist,
    },
    Bool {
        init: bool,
    },
    Sub {
        optional: bool,
        map: HashMap<String, Box<Node>>,
    },
    AnonMap {
        optional: bool,
        value_type: Box<Node>,
        init_size: usize,
        min_size: Option<usize>,
        max_size: Option<usize>,
    },
    ConstInt(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RealProbDist {
    Normal,
    Exponential,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntProbDist {
    Normal,
    Uniform,
}
