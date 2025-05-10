use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiResponse<T> {
    Success { data: T, error: Option<()> },
    Error { data: Option<()>, error: String },
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self::Success { data, error: None }
    }

    pub fn error(error: impl Into<String>) -> Self {
        Self::Error {
            data: None,
            error: error.into(),
        }
    }
}

// Helper trait to convert any type into our ApiResponse
pub trait IntoApiResponse<T> {
    fn into_api_response(self) -> ApiResponse<T>;
}

// Implement for Result type
impl<T, E: ToString> IntoApiResponse<T> for Result<T, E> {
    fn into_api_response(self) -> ApiResponse<T> {
        match self {
            Ok(data) => ApiResponse::success(data),
            Err(error) => ApiResponse::error(error.to_string()),
        }
    }
}

// Implement for Option type
impl<T> IntoApiResponse<T> for Option<T> {
    fn into_api_response(self) -> ApiResponse<T> {
        match self {
            Some(data) => ApiResponse::success(data),
            None => ApiResponse::error("Not found"),
        }
    }
}

// Type-safe wrapper for axum responses that enforces our API response format
pub struct ApiResult<T>(pub ApiResponse<T>);

impl<T: Serialize> IntoResponse for ApiResult<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

// Helper to create a type-safe API response
pub fn api_response<T>(response: impl IntoApiResponse<T>) -> ApiResult<T> {
    ApiResult(response.into_api_response())
}
