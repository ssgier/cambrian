use crate::error::Error;
use crate::event::ControllerEvent;
use crate::event::IndividualEvalJob;
use crate::meta::AsyncObjectiveFunction;
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use futures::SinkExt;
use log::trace;
use std::sync::Arc;

pub async fn start_worker<F: AsyncObjectiveFunction>(
    obj_func: Arc<F>,
    mut event_sender: UnboundedSender<ControllerEvent>,
) -> Result<(), Error> {
    trace!("Worker starting");

    let mut job_receiver: Option<oneshot::Receiver<IndividualEvalJob>>;
    let (sender, job_recv) = oneshot::channel::<IndividualEvalJob>();
    job_receiver = Some(job_recv);

    event_sender
        .send(ControllerEvent::WorkerReady {
            eval_job_sender: sender,
        })
        .await
        .ok();

    while let Ok(job) = job_receiver.take().unwrap().await {
        let eval_result = obj_func.evaluate(job.individual.to_json()).await;

        let (job_sender, job_recv) = oneshot::channel::<IndividualEvalJob>();
        job_receiver = Some(job_recv);
        event_sender
            .send(ControllerEvent::IndividualEvalCompleted {
                obj_func_val: eval_result,
                individual_id: job.individual_id,
                next_eval_job_sender: job_sender,
            })
            .await
            .ok();
    }

    event_sender
        .send(ControllerEvent::WorkerTerminating)
        .await
        .ok();

    trace!("Worker terminating");

    Ok(())
}
