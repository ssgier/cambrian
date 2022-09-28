use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Value(pub Node);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Node {
    Real(f64),
    Int(i64),
    Bool(bool),
    Sub(HashMap<String, Box<Node>>),
    AnonMap(HashMap<usize, Box<Node>>),
}
