use crate::error::ProcOutputWithObjFuncArg;
use crate::{error::Error, meta::AsyncObjectiveFunction};
use async_channel;
use async_trait::async_trait;
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use futures::future::Either;
use log::trace;
use nix::errno::Errno;
use nix::sys::signal::{self, Signal};
use nix::sys::wait;
use nix::unistd::Pid;
use serde::Deserialize;
use std::ffi::OsStr;
use std::process::Stdio;
use std::{ffi::OsString, time::Duration};
use tokio::process::Command;

pub struct ObjFuncProcessDef {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub kill_obj_func_after: Option<Duration>,
}

impl ObjFuncProcessDef {
    pub fn new(
        program: OsString,
        args: Vec<OsString>,
        kill_obj_func_after: Option<Duration>,
    ) -> Self {
        Self {
            program,
            args,
            kill_obj_func_after,
        }
    }
}

fn kill_and_reap_child_proc_group(unreaped_pgid: Option<Pid>) -> Result<(), Error> {
    if let Some(pgid) = unreaped_pgid {
        match signal::killpg(pgid, Signal::SIGKILL) {
            Err(Errno::ESRCH) => Ok(()),
            Err(_) => Err(Error::FailedToKillChildProcessGroup(pgid)),
            Ok(()) => match wait::waitpid(pgid, None) {
                Ok(_) => Ok(()),
                Err(_) => Err(Error::FailedToReapChildProcessGroup(pgid)),
            },
        }
    } else {
        Ok(())
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct ObjFuncChildResult {
    objFuncVal: Option<f64>,
}

async fn get_child_result(
    child: AsyncGroupChild,
    obj_func_arg: &OsStr,
) -> Result<Option<f64>, Error> {
    let output = child.wait_with_output().await?;

    if output.status.success() {
        let result: ObjFuncChildResult = serde_json::from_slice(&output.stdout).map_err(|_| {
            Error::ObjFuncProcInvalidOutput(ProcOutputWithObjFuncArg::new(
                obj_func_arg.to_owned(),
                output,
            ))
        })?;
        Ok(result.objFuncVal)
    } else {
        trace!(
            "Child terminated unsuccessfully, status: {:?}",
            output.status
        );
        Err(Error::ObjFuncProcFailed(ProcOutputWithObjFuncArg::new(
            obj_func_arg.to_owned(),
            output,
        )))
    }
}

#[async_trait]
impl AsyncObjectiveFunction for ObjFuncProcessDef {
    async fn evaluate(
        &self,
        value: serde_json::Value,
        abort_sig_rx: async_channel::Receiver<()>,
    ) -> Result<Option<f64>, Error> {
        let json_arg: OsString = serde_json::to_string(&value).unwrap().into();
        let child = Command::new(&self.program)
            .args(&self.args)
            .arg(&json_arg)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .group_spawn()?;

        let unreaped_pgid = child.id().map(|pgid| Pid::from_raw(pgid as i32));

        let child_result = get_child_result(child, &json_arg);

        let mut timeout_fut = if let Some(kill_after_duration) = self.kill_obj_func_after {
            let timeout_fut = Box::pin(tokio::time::sleep(kill_after_duration));
            Either::Left(timeout_fut)
        } else {
            Either::Right(futures::future::pending())
        };

        tokio::pin!(child_result);

        let abort_sig_future = abort_sig_rx.recv();

        tokio::select! {
            result = &mut child_result => {
                return result
            }
            _ = &mut timeout_fut => {
                kill_and_reap_child_proc_group(unreaped_pgid)?;
                return Ok(None)
            }
            _ = abort_sig_future => {
                kill_and_reap_child_proc_group(unreaped_pgid)?;
                return Ok(None)
            }
        }
    }
}
