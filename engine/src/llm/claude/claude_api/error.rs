use thiserror::Error;

/// Errors returned by the Claude / Anthropic API
#[derive(Debug, Error)]
pub enum ClaudeApiError {
    #[error("Invalid request (400): {message}")]
    InvalidRequest { message: String },

    #[error("Authentication error (401): {message}")]
    Authentication { message: String },

    #[error("Permission error (403): {message}")]
    Permission { message: String },

    #[error("Not found (404): {message}")]
    NotFound { message: String },

    #[error("Request too large (413): {message}")]
    RequestTooLarge { message: String },

    #[error("Rate limit exceeded (429): {message}")]
    RateLimit { message: String },

    #[error("Internal API error (500): {message}")]
    Api { message: String },

    #[error("API overloaded (529): {message}")]
    Overloaded { message: String },

    /// Catch-all for unexpected status codes
    #[error("Unexpected API error: {message}")]
    Unexpected { error_type: String, message: String },
}

impl ClaudeApiError {
    pub fn from_type(error_type: &str, message: impl Into<String>) -> Self {
        let message = message.into();

        match error_type {
            "invalid_request_error" => Self::InvalidRequest { message },
            "authentication_error" => Self::Authentication { message },
            "permission_error" => Self::Permission { message },
            "not_found_error" => Self::NotFound { message },
            "request_too_large" => Self::RequestTooLarge { message },
            "rate_limit_error" => Self::RateLimit { message },
            "api_error" => Self::Api { message },
            "overloaded_error" => Self::Overloaded { message },
            other => Self::Unexpected {
                error_type: other.to_string(),
                message,
            },
        }
    }
}
