use crate::meta::CrossoverParams;
use crate::{spec::Spec, value::Value};

trait CrossoverStrategy {
    fn crossover(
        values_ordered: &[Value],
        spec: &Spec,
        crossover_params: &CrossoverParams,
    ) -> Value;
}
