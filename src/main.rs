mod converter;
mod error_responses;
mod overseer;

use crate::error_responses::HttpErrorJson;

use core::future::Future; 
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
use futures::TryStreamExt;
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
};
use std::net::SocketAddr;

/*
type JobType = Box<dyn Future<Output = PathBuf>>;

/// Internal state of the application.
struct AppState {
    next_job_id: usize,
    jobs: Arc<RwLock<HashMap<usize, Job>>>,
}

impl AppState {
    pub fn new() -> AppState {
        AppState {
            next_job_id: usize,
            jobs: HashMap<usize, Job>,
        }
    }

    async fn create_job(
        &mut self,
        path: impl AsRef<OsStr>
    ) -> Job {
    }
}
*/

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

pub async fn on_upload(mut multipart: Multipart) -> Result<Response, StatusCode>
{
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
            eprintln!("Server can't yet handle multiple media to be concatenated.");
            return Ok(HttpErrorJson::unimplemented(Some("Cannot process multiple files for the moment.")));
        }

        // determine the filename
        let filename = match field.file_name() {
            None => return Ok(HttpErrorJson::bad_request(format!("No file name found for file #{}", index))),
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
