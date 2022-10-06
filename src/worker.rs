use crate::error::Error;
use crate::event::ControllerEvent;
use crate::event::IndividualEvalJob;
use crate::meta::ObjectiveFunction;
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use futures::SinkExt;

pub async fn start_worker<F: ObjectiveFunction>(
    mut event_sender: UnboundedSender<ControllerEvent>,
) -> Result<(), Error> {
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
        let eval_result = F::evaluate(&job.individual.to_json()).await;

        let (job_sender, job_recv) = oneshot::channel::<IndividualEvalJob>();
        job_receiver = Some(job_recv);
        event_sender
            .send(ControllerEvent::IndividualEvalCompleted {
                obj_func_val: eval_result,
                individual: job.individual,
                next_eval_job_sender: job_sender,
            })
            .await
            .ok();
    }

    Ok(())
}
