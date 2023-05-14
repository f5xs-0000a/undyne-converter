use std::{
    ffi::OsString,
    path::PathBuf,
    sync::Arc,
};

use futures::{
    future::BoxFuture,
    FutureExt as _,
};
use tokio::sync::mpsc::{
    unbounded_channel,
    UnboundedReceiver,
    UnboundedSender,
};

use crate::{
    converter::{
        AudioConstants,
        JobStatus,
    },
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

#[derive(Debug, Copy, Clone)]
pub enum AudioVideoStatus {
    FirstPass,
    SecondPass,
    Finished,
}

pub struct Job {
    future: BoxFuture<'static, PathBuf>,
    status_receiver: UnboundedReceiver<JobStatus>,
    request_sender: UnboundedSender<RequestForJobStatus>,
    output: Option<PathBuf>,
}

impl Job {
    pub fn new(path: OsString) -> Job {
        let (status_sender, status_receiver) = unbounded_channel();
        let (request_sender, request_receiver) = unbounded_channel();

        let future = crate::converter::actually_run_job(
            path,
            status_sender,
            request_receiver,
        )
        .boxed();

        Job {
            future,
            status_receiver,
            request_sender,
            output: None,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.output.is_some()
    }

    pub async fn run_until_finished(&mut self) {
        if self.output.is_some() {
            return;
        }

        self.output = Some((&mut self.future).await);
    }

    pub async fn request_job_status(&mut self) -> JobStatus {
        self.request_sender.send(RequestForJobStatus);
        self.status_receiver.recv().await.unwrap()
    }
}
