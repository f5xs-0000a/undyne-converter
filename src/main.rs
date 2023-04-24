use std::{
    net::SocketAddr,
};

use axum::{
    body::{
        boxed,
    },
    extract::{
        Multipart,
    },
    http::StatusCode,
    response::Response,
    routing::{
        on,
        MethodFilter,
    },
    Router,
};
use futures::{
    TryStreamExt,
};
use tokio::{
    fs::{
        OpenOptions,
    },
    io::{
        AsyncWriteExt,
    },
};

#[tokio::main]
async fn main() {
    let router = Router::new().route("/upload", on(MethodFilter::POST, upload));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}

pub async fn upload(mut multipart: Multipart) -> Result<Response, StatusCode> {
    while let Some(mut field) = match multipart.next_field().await {
        Err(_e) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) => None,
        Ok(Some(field)) => Some(field),
    } {
        let filename = match field.file_name() {
            None => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            Some(fname) => fname,
        };
        let mut file = match OpenOptions::new()
            .create(true)
            .write(true)
            .open(filename)
            .await
        {
            Err(_e) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            Ok(f) => f,
        };

        while let Some(bytes) = field.try_next().await.unwrap() {
            file.write(&bytes).await.unwrap();
        }
        file.flush().await.unwrap();
    }

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(boxed("Ok".to_owned()))
        .unwrap())
}
