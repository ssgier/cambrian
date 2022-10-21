use anyhow::{Context, Result};
use cambrian::termination::TerminationCriterion;
use cambrian::{meta::AlgoConfigBuilder, process::ObjFuncProcessDef, spec_util, sync_launch};
use clap::Parser;
use parse_duration::parse::parse;
use std::{ffi::OsString, fs, path::PathBuf};

#[derive(Parser, Default, Debug)]
#[command(author = "Sandro Sgier", version, about = "Run asynchronous adaptive genetic algorithm", long_about = None)]
struct Args {
    #[arg(short = 'n', long)]
    max_obj_func_eval: Option<usize>,

    #[arg(short = 'p', long)]
    num_parallel: Option<usize>,

    #[arg(short = 's', long)]
    spec_file: PathBuf,

    #[arg(short, long)]
    out_file: PathBuf,

    #[arg(short = 'k', long)]
    kill_obj_func_after: Option<String>,

    obj_func_program: OsString,
    obj_func_program_args: Vec<OsString>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    env_logger::init();

    let kill_obj_func_after = args
        .kill_obj_func_after
        .map(|kill_after| {
            parse(&kill_after)
                .with_context(|| format!("Unable to parse duration from value \"{}\"", kill_after))
        })
        .transpose()?;

    let spec_ctx = |op| format!("Unable to {} spec_file: {}", op, &args.spec_file.display());
    let spec_str = fs::read_to_string(&args.spec_file).with_context(|| spec_ctx("read"))?;
    let spec = spec_util::from_yaml_str(&spec_str).with_context(|| spec_ctx("parse"))?;

    let process_def = ObjFuncProcessDef::new(
        args.obj_func_program,
        args.obj_func_program_args,
        kill_obj_func_after,
    );

    let mut algo_config_builder = AlgoConfigBuilder::new();

    if let Some(num_parallel) = args.num_parallel {
        algo_config_builder.num_concurrent(num_parallel);
    }

    let algo_config = algo_config_builder.build();

    let mut termination_criteria = Vec::new();

    if let Some(max_obj_func_eval) = args.max_obj_func_eval {
        termination_criteria.push(TerminationCriterion::NumObjFuncEval(max_obj_func_eval))
    }

    sync_launch::launch_with_async_obj_func(spec, process_def, algo_config, termination_criteria)
        .context("Algorithm run failed")?;

    Ok(())
}
