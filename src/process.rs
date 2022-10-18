use crate::{error::Error, meta::AsyncObjectiveFunction};
use async_trait::async_trait;
use futures::pin_mut;
use futures::FutureExt;
use futures_timer::Delay;
use log::trace;
use serde::Deserialize;
use std::process::Stdio;
use std::{ffi::OsString, time::Duration};

use async_process::{Child, Command};

pub struct ObjFuncProcessDef {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub kill_after: Option<Duration>,
}

impl ObjFuncProcessDef {
    pub fn new(program: OsString, args: Vec<OsString>, kill_after: Option<Duration>) -> Self {
        Self {
            program,
            args,
            kill_after,
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct ObjFuncChildResult {
    objFuncVal: Option<f64>,
}

async fn get_child_result(child: Child) -> Result<Option<f64>, Error> {
    let output = child.output().await?;
    if output.status.success() {
        let result: ObjFuncChildResult = serde_json::from_slice(&output.stdout)?;
        Ok(result.objFuncVal)
    } else {
        trace!(
            "Child terminated unsuccessfully, status: {:?}",
            output.status
        );
        Err(Error::ChildUnsuccessfulTermination(output))
    }
}

#[async_trait]
impl AsyncObjectiveFunction for ObjFuncProcessDef {
    async fn evaluate(&self, value: serde_json::Value) -> Result<Option<f64>, Error> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .arg(serde_json::to_string(&value).unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        trace!("Spawned objective function process, pid: {:?}", child.id());

        match self.kill_after {
            None => get_child_result(child).await,
            Some(kill_after_duration) => {
                let timeout_fut = Delay::new(kill_after_duration).fuse();
                let status_fut = child.status().fuse();
                pin_mut!(timeout_fut, status_fut);
                futures::select! {
                    () = &mut timeout_fut => {
                        trace!("Timeout on child with PID {:?}. Killing.", child.id());
                        child.kill().ok();
                        child.status().await?;
                        Ok(None)
                    }
                    _ = status_fut => get_child_result(child).await
                }
            }
        }
    }
}
