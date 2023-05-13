mod converter;
mod error_responses;
mod overseer;

use core::future::Future;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
};

use axum::{
    body::boxed,
    extract::Multipart,
    http::StatusCode,
    response::Response,
    routing::{
        on,
        MethodFilter,
    },
    Router,
};
use futures::{
    future::BoxFuture,
    TryStreamExt,
};
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    select,
    sync::mpsc::Receiver,
};

use crate::error_responses::HttpErrorJson;

struct Job {
    future: BoxFuture<'static, PathBuf>,
    output: Option<PathBuf>,
}

impl Job {
    pub fn new() -> Job {
        unimplemented!()
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
}

struct AppState {
    server_requests: Receiver<()>,
    jobs: HashMap<usize, Job>,
}

impl AppState {
    pub fn new() -> AppState {
        unimplemented!()
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
                maybe_message = self.server_requests.recv() => {
                    let message = match maybe_message {
                        Some(m) => m,
                        None => break,
                    };

                    // TODO: actually read the message

                    self.jobs.insert(0, Job::new());

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

#[tokio::main]
async fn main() {
    let state = unimplemented!();

    let router = Router::new()
        .route("/upload", on(MethodFilter::POST, on_upload))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}

pub async fn on_upload(
    mut multipart: Multipart
) -> Result<Response, StatusCode> {
    eprintln!("Received file upload request.");

    let mut files: Vec<()> = vec![];

    // for every file that exists in the field
    let mut index = 0;
    while let Some((mut field, index)) = match multipart.next_field().await {
        // TODO: not everything is an internal server error
        Err(e) => return Ok(HttpErrorJson::bad_multipart(index)),
        Ok(None) => None,
        Ok(Some(field)) => {
            // update the file index
            let cur_index = index;
            index += 1;
            Some((field, index))
        },
    } {
        if files.len() > 1 {
            eprintln!(
                "Server can't yet handle multiple media to be concatenated."
            );
            return Ok(HttpErrorJson::unimplemented(Some(
                "Cannot process multiple files for the moment.",
            )));
        }

        // determine the filename
        let filename = match field.file_name() {
            None => {
                return Ok(HttpErrorJson::bad_request(format!(
                    "No file name found for file #{}",
                    index
                )))
            },
            Some(fname) => fname.to_owned(),
        };

        // create the file
        let mut file = match OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .await
        {
            Err(_e) => return Ok(HttpErrorJson::internal_server_error(None)),
            Ok(f) => f,
        };

        // write the file
        {
            while let Some(bytes) = field.try_next().await.unwrap() {
                // TODO: an appropriate error
                file.write(&bytes).await.unwrap();
            }
            // TODO: also an appropriate error
            file.flush().await.unwrap();

            // TODO: this should be an async block to receive a file. once a
            // file has been downloaded, a process should check the file if it
            // is a valid file

            // one way to do this is to use two processors in a join, one to
            // process sequential downloads and the other to check if the file
            // downloaded is a valid file
        }

        eprintln!("Written {}", filename);
    }

    // ...to whom do you send the file?
    // you should be able to send the file to somewhere else. a service that
    // continually runs beside the web server

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(boxed("Ok".to_owned()))
        .unwrap())
}
