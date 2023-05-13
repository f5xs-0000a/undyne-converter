use std::collections::HashMap;

use tokio::{
    select,
    sync::{
        mpsc::{
            UnboundedReceiver,
            UnboundedSender,
        },
        oneshot::Receiver as OneshotReceiver,
    },
};

use crate::overseer::Job;

pub struct AppState {
    requests_to_app: UnboundedReceiver<()>,
    responses_to_server: UnboundedSender<()>,
    jobs: HashMap<usize, Job>,
}

#[derive(Clone)]
pub struct AppStateMessenger {
    sender_to_state: UnboundedSender<()>,
}

impl AppStateMessenger {
    pub fn send_message_expecting_response(&self) -> OneshotReceiver<()> {
        unimplemented!()
    }
}

impl AppState {
    pub fn new() -> (AppState, AppStateMessenger) {
        let state = AppState {
            jobs: HashMap::new(),
            requests_to_app: unimplemented!(),
            responses_to_server: unimplemented!(),
        };

        (state, unimplemented!())
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
                    let message = match maybe_message {
                        Some(m) => m,
                        None => break,
                    };

                    // TODO: actually read the message

                    self.jobs.insert(0, unimplemented!());

                    all_has_finished = false;
                },

                // only check all of the jobs if not all of them are finished
                _ = joined_check, if !all_has_finished => {
                    all_has_finished = true;
                },
            }
        }
    }
}
