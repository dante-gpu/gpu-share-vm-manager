use axum::{http::Request, middleware::Next, response::Response};
use headers::{Authorization, HeaderMapExt};
use jsonwebtoken::{decode, DecodingKey, Validation};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
    role: String,
    exp: usize,
}

pub async fn auth_middleware<B>(
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let token = req.headers()
        .typed_get::<Authorization<Bearer>>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token.token(),
        &DecodingKey::from_secret(std::env::var("JWT_SECRET").unwrap().as_ref()),
        &validation,
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(token_data.claims);
    Ok(next.run(req).await)
} 