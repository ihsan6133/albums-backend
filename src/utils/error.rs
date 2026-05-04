use std::error::Error;

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    code: u16,
    description: String
}
pub struct HttpError(pub StatusCode, pub ErrorResponse);

impl HttpError {
    pub fn new(status: StatusCode, description: String) -> Self {
        HttpError(status, ErrorResponse { code: status.as_u16(), description })
    }

    pub fn from_code(status: StatusCode) -> Self {
        HttpError(status, ErrorResponse { code: status.as_u16(), description: status.canonical_reason().unwrap_or("Unknown error").to_string() })
    }
}


impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, error) = (self.0, self.1);
        let body = Json(error);
        (status, body).into_response()
    }
}


pub trait ToHttpError<T> {
    fn map_http(self, status: StatusCode) -> Result<T, HttpError>;
}

impl <T, E: Error> ToHttpError<T> for Result<T, E> {
    fn map_http(self, status: StatusCode) -> Result<T, HttpError> {
        self.map_err(|e| HttpError(status, ErrorResponse { code: status.as_u16(), description: e.to_string() }))
    }
}

