use anyhow::{Context, Result};
use cambrian::error::{Error, ProcOutputWithObjFuncArg};
use cambrian::meta::AlgoConfig;
use cambrian::result::FinalReport;
use cambrian::spec::Spec;
use cambrian::sync_launch::DetailedReportingFileInfo;
use cambrian::termination::TerminationCriterion;
use cambrian::{meta::AlgoConfigBuilder, process::ObjFuncProcessDef, spec_util, sync_launch};
use clap::Parser;
use log::{info, LevelFilter};
use parse_duration::parse::parse;
use std::fmt::Write;
use std::os::unix::prelude::OsStrExt;
use std::path::Path;
use std::{ffi::OsString, fs, path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
#[command(author, version, about = "Run asynchronous adaptive genetic algorithm", long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,

    #[arg(short, long)]
    force: bool,

    #[arg(long)]
    no_signal_result: bool,

    #[arg(short = 'n', long)]
    max_obj_func_eval: Option<usize>,

    #[arg(short, long)]
    target_obj_func_val: Option<f64>,

    #[arg(long)]
    terminate_after: Option<String>,

    #[arg(long)]
    num_concurrent: Option<usize>,

    #[arg(short = 's', long)]
    spec_file: PathBuf,

    #[arg(short, long)]
    out_dir: Option<PathBuf>,

    #[arg(short = 'k', long)]
    kill_obj_func_after: Option<String>,

    #[arg(long)]
    sample_size: Option<usize>,

    #[arg(long)]
    quantile: Option<f64>,

    #[arg(long)]
    initial_guess: Option<String>,

    obj_func_program: OsString,
    obj_func_program_args: Vec<OsString>,
}

fn init_logger(args: &Args) {
    let level_filter = if args.verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Error
    };

    env_logger::Builder::new()
        .filter_level(level_filter)
        .format_timestamp(None)
        .format_level(false)
        .format_module_path(false)
        .format_target(false)
        .init();
}

fn load_spec(args: &Args) -> Result<Spec> {
    let spec_file_display = args.spec_file.display();
    let spec_ctx = |op| format!("Unable to {} spec_file: {}", op, &spec_file_display);

    info!("Reading spec file: {}", spec_file_display);
    let spec_str = fs::read_to_string(&args.spec_file).with_context(|| spec_ctx("read"))?;
    let spec = spec_util::from_yaml_str(&spec_str).with_context(|| spec_ctx("parse"))?;
    Ok(spec)
}

fn make_algo_conf(args: &Args) -> Result<AlgoConfig> {
    let mut algo_config_builder = AlgoConfigBuilder::new();

    if let Some(num_concurrent) = args.num_concurrent {
        algo_config_builder.num_concurrent(num_concurrent);
    }

    if let Some(sample_size) = args.sample_size {
        algo_config_builder.individual_sample_size(sample_size);
    }

    if let Some(quantile) = args.quantile {
        algo_config_builder.obj_func_val_quantile(quantile);
    }

    algo_config_builder.build().context("invalid input")
}

fn assemble_termination_criteria(args: &Args) -> Result<Vec<TerminationCriterion>> {
    let mut termination_criteria = Vec::new();

    if let Some(max_obj_func_eval) = args.max_obj_func_eval {
        termination_criteria.push(TerminationCriterion::NumObjFuncEval(max_obj_func_eval))
    }

    if let Some(target_obj_func_val) = args.target_obj_func_val {
        termination_criteria.push(TerminationCriterion::TargetObjFuncVal(target_obj_func_val));
    }

    if let Some(ref terminate_after) = args.terminate_after {
        let terminate_after =
            parse_duration(terminate_after).context("Unable to parse \"terminate_after\"")?;

        termination_criteria.push(TerminationCriterion::TerminateAfter(terminate_after));
    }

    if !args.no_signal_result {
        termination_criteria.push(TerminationCriterion::Signal)
    }

    Ok(termination_criteria)
}

fn make_obj_func_def(
    obj_func_program: OsString,
    obj_func_program_args: Vec<OsString>,
    kill_obj_func_after: Option<String>,
) -> Result<ObjFuncProcessDef> {
    let kill_obj_func_after = kill_obj_func_after
        .as_ref()
        .map(|kill_after| {
            parse_duration(kill_after).context("Unable to parse \"kill objective function after\"")
        })
        .transpose()?;

    Ok(ObjFuncProcessDef::new(
        obj_func_program,
        obj_func_program_args,
        kill_obj_func_after,
    ))
}

fn parse_duration(value: &str) -> Result<Duration> {
    parse(value).with_context(|| format!("Unable to parse duration from value \"{}\"", value))
}

fn process_report(report: FinalReport, out_dir: &Option<PathBuf>) -> Result<()> {
    if let Some(out_dir) = out_dir {
        write_file(
            &out_dir.join("summary_report.txt"),
            "report",
            report.to_string().as_bytes(),
        )?;
    }

    println!("{}", report.best_seen.value);

    Ok(())
}

fn write_file(path: &PathBuf, descr: &str, content: &[u8]) -> Result<()> {
    info!("Writing {} to file: {}", descr, path.display());
    fs::write(&path, content).with_context(|| format!("Unable to write {} file", descr))
}

fn dump_diagnostic_files(
    dump_info: &DiagnosticDumpFileInfo,
    proc_info: &ProcOutputWithObjFuncArg,
) -> Result<()> {
    write_file(
        &dump_info.failed_obj_func_arg_file_path,
        "failed objective function argument",
        proc_info.obj_func_arg.as_bytes(),
    )?;

    write_file(
        &dump_info.failed_obj_func_stdout_file_path,
        "failed objective function stdout",
        &proc_info.output.stdout,
    )?;

    write_file(
        &dump_info.failed_obj_func_stderr_file_path,
        "failed objective function stderr",
        &proc_info.output.stderr,
    )?;

    Ok(())
}

fn extract_diagnostic_info<'a>(
    out_dir: &Option<PathBuf>,
    result: &'a Result<FinalReport, Error>,
) -> Option<(DiagnosticDumpFileInfo, &'a ProcOutputWithObjFuncArg)> {
    match out_dir {
        Some(out_dir) => match result {
            Err(Error::ObjFuncProcFailed(proc_out_with_arg))
            | Err(Error::ObjFuncProcInvalidOutput(proc_out_with_arg)) => {
                Some((DiagnosticDumpFileInfo::new(out_dir), proc_out_with_arg))
            }
            _ => None,
        },
        None => None,
    }
}

fn handle_existing_out_dir(out_dir: &PathBuf, force: bool) -> Result<()> {
    let out_dir_exists = out_dir.try_exists().with_context(|| {
        format!(
            "Tried to check if output directory already exists, but failed: {}",
            out_dir.display()
        )
    })?;

    if out_dir_exists {
        if force {
            info!(
                "Used -f or --force: removing pre-existing output directory: {}",
                out_dir.display()
            );
            fs::remove_dir_all(out_dir)
                .context("Failed to remove pre-existing output directory")?;
        } else {
            Err(Error::OutputDirectoryAlreadyExists)
                .context("Output directory alredy exists. Using -f or --force will remove it")?;
        }
    }

    info!("Creating output directory: {}", out_dir.display());
    fs::create_dir_all(out_dir).context("Unable to create output directory")?;

    Ok(())
}

fn make_detailed_reporting_file_info(args: &Args) -> Option<DetailedReportingFileInfo> {
    args.out_dir.as_ref().map(|out_dir| {
        let detailed_report_file_path = out_dir.join("detailed_report.csv");
        let best_seen_file_path = out_dir.join("best_seen.json");

        DetailedReportingFileInfo {
            detailed_report_file_path,
            best_seen_file_path,
        }
    })
}

struct DiagnosticDumpFileInfo {
    failed_obj_func_arg_file_path: PathBuf,
    failed_obj_func_stdout_file_path: PathBuf,
    failed_obj_func_stderr_file_path: PathBuf,
}

impl DiagnosticDumpFileInfo {
    fn new(out_dir: &Path) -> Self {
        Self {
            failed_obj_func_arg_file_path: out_dir.join("failed_obj_func_arg"),
            failed_obj_func_stdout_file_path: out_dir.join("failed_obj_func_stdout"),
            failed_obj_func_stderr_file_path: out_dir.join("failed_obj_func_stderr"),
        }
    }
}

fn provide_context(
    result: Result<FinalReport, Error>,
    diagnostic_file_info: Option<DiagnosticDumpFileInfo>,
    detailed_reporting_file_info: Option<DetailedReportingFileInfo>,
) -> Result<FinalReport> {
    result.with_context(|| {
        let mut descr = "Algorithm run failed.".to_string();

        if let Some(file_info) = diagnostic_file_info {
            write!(
                &mut descr,
                r"
Argument passed to failed objective function process can be found in file: {}",
                file_info.failed_obj_func_arg_file_path.display()
            )
            .unwrap();

            write!(
                &mut descr,
                r"
Standard output of failed objective function process can be found in file: {}",
                file_info.failed_obj_func_stdout_file_path.display()
            )
            .unwrap();

            write!(
                &mut descr,
                r"
Standard error output of failed objective function process can be found in file: {}",
                file_info.failed_obj_func_stderr_file_path.display()
            )
            .unwrap();
        }

        if let Some(file_info) = detailed_reporting_file_info {
            write!(
                &mut descr,
                r"
Detailed reporting information can be found in file: {}",
                file_info.detailed_report_file_path.display()
            )
            .unwrap();

            if file_info.best_seen_file_path.exists() {
                write!(
                    &mut descr,
                    r"
Best seen value can be found in file: {}",
                    file_info.best_seen_file_path.display()
                )
                .unwrap();
            }
        }

        descr
    })
}

fn main() -> Result<()> {
    let args = Args::parse();

    init_logger(&args);

    let algo_config = make_algo_conf(&args)?;
    let termination_criteria = assemble_termination_criteria(&args)?;

    if let Some(out_dir) = &args.out_dir {
        handle_existing_out_dir(out_dir, args.force)?;
    }

    let detailed_reporting_file_info = make_detailed_reporting_file_info(&args);

    let spec = load_spec(&args)?;
    let obj_func_def = make_obj_func_def(
        args.obj_func_program,
        args.obj_func_program_args,
        args.kill_obj_func_after,
    )?;

    let explicit_init_value_json = args
        .initial_guess
        .map(|json_str| serde_json::from_str(&json_str))
        .transpose()
        .context("Failed to parse initial guess JSON. Check validity.")?;

    let result = sync_launch::launch_with_async_obj_func(
        spec,
        obj_func_def,
        algo_config,
        termination_criteria,
        explicit_init_value_json,
        false,
        detailed_reporting_file_info.as_ref(),
    );

    let diagnostic_info = extract_diagnostic_info(&args.out_dir, &result);

    if let Some((file_info, proc_info)) = &diagnostic_info {
        dump_diagnostic_files(file_info, proc_info)?;
    }

    let diagnostic_file_info = diagnostic_info.map(|info| info.0);

    let report = provide_context(result, diagnostic_file_info, detailed_reporting_file_info)?;

    process_report(report, &args.out_dir)?;
    info!("Done");
    Ok(())
}
