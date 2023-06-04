mod converter;
mod error_responses;
mod job_manager;
mod overseer;
mod query_string;

use std::{
    ffi::OsString,
    net::SocketAddr,
};

use axum::{
    body::boxed,
    extract::{
        Multipart,
        State,
    },
    http::{
        StatusCode,
        Uri,
    },
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

use crate::{
    error_responses::HttpErrorJson,
    job_manager::{
        AppState,
        AppStateMessenger,
        MessageFromServerToApp,
    },
};

#[tokio::main]
async fn main() {
    let (mut app_state, app_state_messenger) = AppState::new();

    let router = Router::new()
        .route("/upload", on(MethodFilter::POST, on_multipart_upload))
        .with_state(app_state_messenger);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    // > the StateMessenger sent into the web server is just a messenger to the
    //   actual AppState. this is held by axum.
    // > the AppState is run as a future. aggregates jobs run and also receives
    //   messages on which
    //   > jobs to run
    //   > which jobs to ask status for
    //   > which jobs to delete
    // > the JobWrapper is a wrapper for an actual job. this contains messenger
    //   towards the actual job future

    let web_server_future =
        axum::Server::bind(&addr).serve(router.into_make_service());

    eprintln!("Now running at {}", addr);

    tokio::select! {
        web_server_end = web_server_future => { web_server_end.unwrap(); },
        _ = app_state.async_loop() => {},
    };
}

/// Behavior for the web server when receiving a multipart upload request.
async fn on_multipart_upload(
    state: State<AppStateMessenger>,
    uri: Uri,
    mut multipart: Multipart,
) -> Response {
    eprintln!("Received file upload request.");

    let mut files: Vec<OsString> = vec![];

    dbg!(());

    let qsc = match query_string::get_requests(uri.query().unwrap_or("")) {
        Ok(qsc) => qsc,
        Err(e) => return HttpErrorJson::bad_request(e.as_error_msg()),
    };

    dbg!(qsc);

    // TODO: delete me
    return Response::builder()
        .status(StatusCode::CREATED)
        .body(boxed("Ok".to_owned()))
        .unwrap();

    // for every file that exists in the field
    let mut index = 0;
    while let Some((mut field, index)) = match multipart.next_field().await {
        Err(e) => return HttpErrorJson::bad_multipart(index),
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
            return HttpErrorJson::unimplemented(Some(
                "Cannot process multiple files for the moment.",
            ));
        }

        // determine the filename
        let filename = match field.file_name() {
            None => {
                return HttpErrorJson::bad_request(format!(
                    "No file name found for file #{}",
                    index
                ))
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
            Err(_e) => return HttpErrorJson::internal_server_error(None),
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

        files.push(filename.into());
    }

    // TODO: soon, you'll be able to take more files
    let file = files.into_iter().next().unwrap();
    state
        .0
        .send_message_expecting_response(MessageFromServerToApp::NewJob(file));

    Response::builder()
        .status(StatusCode::CREATED)
        .body(boxed("Ok".to_owned()))
        .unwrap()
}
