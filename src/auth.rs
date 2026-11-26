use axum::{
    extract::{Query, Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;

use crate::{models::Claims, AppState};

#[derive(Debug, Deserialize)]
pub struct StreamTokenQuery {
    pub token: Option<String>,
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, StatusCode> {
    let validation = Validation::default();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| StatusCode::UNAUTHORIZED)
}

pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    Query(query): Query<StreamTokenQuery>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let bearer_token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());

    let token = bearer_token
        .or(query.token)
        .filter(|t| !t.is_empty())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = decode_jwt(&token, &state.jwt_secret)?;
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
