use askama::Template;
use askama_web::WebTemplate;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::error::LocalIndexError;

/// Application error type for dashboard handlers.
pub enum AppError {
    Search(LocalIndexError),
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Search(e) => write!(f, "{}", e),
            AppError::Internal(msg) => write!(f, "{}", msg),
        }
    }
}

impl From<LocalIndexError> for AppError {
    fn from(e: LocalIndexError) -> Self {
        AppError::Search(e)
    }
}

/// Error page template extending base.html.
#[derive(Template, WebTemplate)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
    active_nav: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = self.to_string();
        let template = ErrorTemplate {
            message,
            active_nav: "",
        };
        match template.render() {
            Ok(html) => (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response(),
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<h1>500 Internal Server Error</h1>".to_string()),
            )
                .into_response(),
        }
    }
}
