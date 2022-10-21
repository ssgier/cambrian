use anyhow::{Context, Result};
use cambrian::meta::AlgoConfig;
use cambrian::spec::Spec;
use cambrian::termination::TerminationCriterion;
use cambrian::{meta::AlgoConfigBuilder, process::ObjFuncProcessDef, spec_util, sync_launch};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
use log::info;
use parse_duration::parse::parse;
use std::{ffi::OsString, fs, path::PathBuf};

#[derive(Parser, Debug)]
#[command(author, version, about = "Run asynchronous adaptive genetic algorithm", long_about = None)]
struct Args {
    #[clap(flatten)]
    verbose: Option<Verbosity>,

    #[arg(short = 'n', long)]
    max_obj_func_eval: Option<usize>,

    #[arg(short = 'p', long)]
    num_parallel: Option<usize>,

    #[arg(short = 's', long)]
    spec_file: PathBuf,

    #[arg(short, long)]
    out_file: Option<PathBuf>,

    #[arg(short = 'k', long)]
    kill_obj_func_after: Option<String>,

    obj_func_program: OsString,
    obj_func_program_args: Vec<OsString>,
}

fn init_logger(args: &Args) {
    env_logger::Builder::new()
        .filter_level(
            args.verbose
                .as_ref()
                .map(|v| v.log_level_filter())
                .unwrap_or_else(|| log::LevelFilter::Error),
        )
        .format_module_path(false)
        .format_target(false)
        .init();
}

fn load_spec(args: &Args) -> Result<Spec> {
    let spec_file_display = args.spec_file.display();
    let spec_ctx = |op| format!("Unable to {} spec_file: {}", op, &spec_file_display);

    info!("Reading spec file: {}", spec_file_display);
    let spec_str = fs::read_to_string(&args.spec_file).with_context(|| spec_ctx("read"))?;

    info!("Parsing spec file");
    let spec = spec_util::from_yaml_str(&spec_str).with_context(|| spec_ctx("parse"))?;
    Ok(spec)
}

fn make_algo_conf(args: &Args) -> AlgoConfig {
    let mut algo_config_builder = AlgoConfigBuilder::new();

    if let Some(num_parallel) = args.num_parallel {
        algo_config_builder.num_concurrent(num_parallel);
    }

    algo_config_builder.build()
}

fn assemble_termination_criteria(args: &Args) -> Vec<TerminationCriterion> {
    let mut termination_criteria = Vec::new();

    if let Some(max_obj_func_eval) = args.max_obj_func_eval {
        termination_criteria.push(TerminationCriterion::NumObjFuncEval(max_obj_func_eval))
    }

    termination_criteria
}

fn make_obj_func_def(
    obj_func_program: OsString,
    obj_func_program_args: Vec<OsString>,
    kill_obj_func_after: Option<String>,
) -> Result<ObjFuncProcessDef> {
    let kill_obj_func_after = kill_obj_func_after
        .as_ref()
        .map(|kill_after| {
            parse(kill_after)
                .with_context(|| format!("Unable to parse duration from value \"{}\"", kill_after))
        })
        .transpose()?;

    Ok(ObjFuncProcessDef::new(
        obj_func_program,
        obj_func_program_args,
        kill_obj_func_after,
    ))
}

fn main() -> Result<()> {
    let args = Args::parse();

    init_logger(&args);
    let spec = load_spec(&args)?;
    let algo_config = make_algo_conf(&args);
    let termination_criteria = assemble_termination_criteria(&args);
    let obj_func_def = make_obj_func_def(
        args.obj_func_program,
        args.obj_func_program_args,
        args.kill_obj_func_after,
    )?;

    let result = sync_launch::launch_with_async_obj_func(
        spec,
        obj_func_def,
        algo_config,
        termination_criteria,
    )
    .context("Algorithm run failed")?;

    if let Some(out_file) = args.out_file {
        info!("Writing result to output file: {}", out_file.display());
        fs::write(&out_file, result.best_seen.value.to_string())
            .with_context(|| format!("Unable to write output file: {}", &out_file.display()))?;
    } else {
        println!("{}", result.best_seen.value);
    }

    info!("Done");

    Ok(())
}
