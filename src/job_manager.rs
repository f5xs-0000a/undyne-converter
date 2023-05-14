use std::{
    collections::HashMap,
    ffi::OsString,
};

use tokio::{
    select,
    sync::{
        mpsc::{
            unbounded_channel as unbounded,
            UnboundedReceiver,
            UnboundedSender,
        },
        oneshot::{
            channel as oneshot,
            Receiver as OneshotReceiver,
            Sender as OneshotSender,
        },
    },
};

use crate::{
    converter::JobStatus,
    overseer::Job,
};

pub enum ResponseFromAppToServer {
    Acknowledged,
    Deleted,
    Status(JobStatus),
    NoSuchJob(usize),
    DeleteRequestIgnored(usize),
}

pub enum MessageFromServerToApp {
    NewJob(OsString),
    StatusRequest(usize),
    DeleteJob(usize, bool), // id, force
}

#[derive(Clone)]
pub struct AppStateMessenger {
    sender_to_state: UnboundedSender<(
        MessageFromServerToApp,
        OneshotSender<ResponseFromAppToServer>,
    )>,
}

impl AppStateMessenger {
    pub fn send_message_expecting_response(
        &self,
        message: MessageFromServerToApp,
    ) -> OneshotReceiver<ResponseFromAppToServer> {
        let (sender, receiver) = oneshot();

        self.sender_to_state.send((message, sender));
        receiver
    }
}

pub struct AppState {
    requests_to_app: UnboundedReceiver<(
        MessageFromServerToApp,
        OneshotSender<ResponseFromAppToServer>,
    )>,
    jobs: HashMap<usize, Job>,
}

impl AppState {
    pub fn new() -> (AppState, AppStateMessenger) {
        let (sender, receiver) = unbounded();

        let state = AppState {
            jobs: HashMap::new(),
            requests_to_app: receiver,
        };

        let messenger = AppStateMessenger {
            sender_to_state: sender,
        };

        (state, messenger)
    }

    fn get_new_job_id(&self) -> usize {
        use rand::distributions::Distribution as _;

        // TODO: you still have to check for an unused ID, even if there is a
        // practically zero chance of collision
        rand::distributions::Standard.sample(&mut rand::rngs::OsRng)
    }

    async fn process_message(
        &mut self,
        message: MessageFromServerToApp,
        rsvp: OneshotSender<ResponseFromAppToServer>,
    ) {
        use MessageFromServerToApp::*;
        use ResponseFromAppToServer::*;

        match message {
            StatusRequest(job_id) => {
                match self.jobs.get_mut(&job_id) {
                    None => drop(rsvp.send(NoSuchJob(job_id))),
                    Some(job) => {
                        let job_status = job.request_job_status().await;
                        drop(rsvp.send(Status(job_status)));
                    },
                }
            },

            NewJob(path) => {
                let new_job = Job::new(path);
                let new_id = self.get_new_job_id();

                self.jobs.insert(new_id, new_job);

                rsvp.send(Acknowledged);
            },

            DeleteJob(id, force) => {
                let job = match self.jobs.remove(&id) {
                    Some(job) => job,
                    None => {
                        drop(rsvp.send(NoSuchJob(id)));
                        return;
                    },
                };

                if job.is_finished() || force {
                    rsvp.send(Deleted);
                }
                else {
                    rsvp.send(DeleteRequestIgnored(id));
                    self.jobs.insert(id, job);
                }
            },
        }
    }

    pub async fn async_loop(&mut self) {
        let mut all_has_finished = false;

        loop {
            let joinable = self
                .jobs
                .iter_mut()
                .map(|(_, v)| v)
                .filter(|job| !job.is_finished())
                .map(|job| job.run_until_finished());
            let joined_check = futures::future::join_all(joinable);

            select! {
                maybe_message = self.requests_to_app.recv() => {
                    let (message, rsvp) = match maybe_message {
                        Some(m) => m,
                        None => break,
                    };

                    self.process_message(message, rsvp).await;
                },

                // only check all of the jobs if not all of them are finished
                _ = joined_check, if !all_has_finished => {
                    all_has_finished = true;
                },
            }
        }
    }
}
