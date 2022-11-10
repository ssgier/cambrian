use self::TerminationCriterion::*;
use crate::error::Error;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum TerminationCriterion {
    NumObjFuncEval(usize),
    TargetObjFuncVal(f64),
    TerminateAfter(Duration),
    Signal,
}

pub(crate) struct CompiledTerminationCriteria {
    pub max_num_obj_func_eval: Option<usize>,
    pub target_obj_func_val: Option<f64>,
    pub terminate_after: Option<Duration>,
    pub terminate_on_signal: bool,
}

pub(crate) fn compile<T>(termination_criteria: T) -> Result<CompiledTerminationCriteria, Error>
where
    T: IntoIterator<Item = TerminationCriterion>,
{
    let mut max_num_obj_func_eval = None;
    let mut target_obj_func_val = None;
    let mut terminate_after = None;
    let mut terminate_on_signal = false;

    for criterion in termination_criteria {
        match criterion {
            NumObjFuncEval(num_eval) if max_num_obj_func_eval.is_none() => {
                max_num_obj_func_eval = Some(num_eval)
            }
            TargetObjFuncVal(target) if target_obj_func_val.is_none() => {
                target_obj_func_val = Some(target)
            }
            TerminateAfter(duration) if terminate_after.is_none() => {
                terminate_after = Some(duration)
            }
            Signal if !terminate_on_signal => terminate_on_signal = true,
            _ => return Err(Error::ConflictingTerminationCriteria),
        }
    }

    Ok(CompiledTerminationCriteria {
        max_num_obj_func_eval,
        target_obj_func_val,
        terminate_after,
        terminate_on_signal,
    })
}
