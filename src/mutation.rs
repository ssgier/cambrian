use crate::meta::MutationParams;
use crate::{spec::Spec, value::Value};

pub trait MutationStrategy {
    fn mutate(value: &Value, spec: &Spec, mutation_params: &MutationParams) -> Value;
}
