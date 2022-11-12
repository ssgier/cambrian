use crate::meta::MetaParamsWrapper;
use std::time::Duration;

#[derive(Clone)]
pub struct DetailedReportItem {
    pub individual_id: usize,
    pub eval_time: Duration,
    pub meta_params_used: Option<MetaParamsWrapper>,
    pub input_val: serde_json::Value,
    pub obj_func_val: Option<f64>,
}

impl DetailedReportItem {
    pub fn get_csv_header_row() -> &'static str {
        "individualId;evalTimeSeconds;metaParamsSource;crossoverProb;selectionPressure;mutationProb;mutationScale;inputVal;objFuncVal\n"
    }

    pub fn to_csv_row(&self) -> String {
        let (meta_params_source, crossover_prob, selection_pressure, mutation_prob, mutation_scale) =
            if let Some(meta_params_wrapper) = &self.meta_params_used {
                (
                    meta_params_wrapper.source.to_string(),
                    meta_params_wrapper
                        .crossover_params
                        .crossover_prob
                        .to_string(),
                    meta_params_wrapper
                        .crossover_params
                        .selection_pressure
                        .to_string(),
                    meta_params_wrapper
                        .mutation_params
                        .mutation_prob
                        .to_string(),
                    meta_params_wrapper
                        .mutation_params
                        .mutation_scale
                        .to_string(),
                )
            } else {
                (
                    String::default(),
                    String::default(),
                    String::default(),
                    String::default(),
                    String::default(),
                )
            };

        let input_val = self.input_val.to_string();
        let obj_func_val = self
            .obj_func_val
            .map(|val| val.to_string())
            .unwrap_or_else(|| "".to_string());

        format!(
            "{};{};{};{};{};{};{};{};{}\n",
            self.individual_id,
            self.eval_time.as_secs_f64(),
            meta_params_source,
            crossover_prob,
            selection_pressure,
            mutation_prob,
            mutation_scale,
            input_val,
            obj_func_val,
        )
    }
}
