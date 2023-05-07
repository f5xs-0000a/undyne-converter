use core::future::Future;
use std::{
    path::PathBuf,
    sync::Arc,
};

use tokio::sync::mpsc::{
    unbounded_channel,
    UnboundedReceiver,
    UnboundedSender,
};

use crate::converter::{
    AudioConstants,
    JobStatus,
};

pub struct RequestForJobStatus;

pub enum JobToOverseerMessage {
    // finished progresses
    //AudioFirstPassFinished,
    AudioSecondPassFinished,
    VideoFirstPassFinished,
    VideoSecondPassFinished,

    AudioConstantsDetermined(Arc<[AudioConstants]>), /* aka AudioFirstPassFinished */
    VideoDimensionsDetermined(usize, usize),
    VideoCrfDetermined(usize),

    VideoSecondPassProgress(PathBuf),
}

#[derive(Copy, Clone)]
pub enum AudioVideoStatus {
    FirstPass,
    SecondPass,
    Finished,
}

/// Runs a job and returns both the future and the receiver for its messages.
pub fn run_job(
    path: PathBuf
) -> (
    impl Future<Output = PathBuf>,
    UnboundedReceiver<JobStatus>,
    UnboundedSender<RequestForJobStatus>,
) {
    let (status_sender, status_receiver) = unbounded_channel();
    let (request_sender, request_receiver) = unbounded_channel();

    (
        crate::converter::actually_run_job(
            path,
            status_sender,
            request_receiver,
        ),
        status_receiver,
        request_sender,
    )
}
