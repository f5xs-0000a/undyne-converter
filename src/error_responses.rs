use axum::{
    http::StatusCode,
    response::{
        IntoResponse as _,
        Response,
    },
};
use serde::Serialize;

#[derive(Serialize)]
pub struct HttpErrorJson {
    code: u16,
    message: String,
}

impl HttpErrorJson {
    pub fn new_response(
        code: StatusCode,
        message: String,
    ) -> Response {
        let content = Self {
            code: code.as_u16(),
            message,
        };

        (code, axum::Json(content)).into_response()
    }

    pub fn bad_multipart(index: usize) -> Response {
        let message = format!(
            "Unable to read file at index {}. Please check if the file exists \
             and try again.",
            index
        );
        Self::new_response(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    pub fn unimplemented(extra_message: Option<&str>) -> Response {
        let mut message = "Process not yet implemented".to_owned();
        if let Some(ex_message) = extra_message {
            message.reserve(ex_message.len() + 2);
            message += ": ";
            message += ex_message;
        }

        Self::new_response(StatusCode::NOT_IMPLEMENTED, message)
    }

    pub fn bad_request(message: String) -> Response {
        Self::new_response(StatusCode::BAD_REQUEST, message)
    }

    pub fn internal_server_error(extra_message: Option<&str>) -> Response {
        let mut message = "Internal server error".to_owned();
        if let Some(ex_message) = extra_message {
            message.reserve(ex_message.len() + 2);
            message += ": ";
            message += &ex_message;
        }

        Self::new_response(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}
