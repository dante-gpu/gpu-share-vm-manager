#[derive(Debug)]
pub struct ErrorResponse {
    pub code: ErrorNumber,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(code: ErrorNumber, message: String) -> Self {
        Self { code, message }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error_code": self.code as u32,
                "message": self.message
            })),
        )
            .into_response()
    }
} 