use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use db::authz;
use jsonwebtoken::{
    decode, decode_header,
    jwk::JwkSet,
    Algorithm, DecodingKey, Validation,
};
use serde::{Deserialize, Serialize};
use std::env;

const X_FORWARDED_ACCESS_TOKEN: &str = "X-Forwarded-Access-Token";

#[derive(Serialize, Deserialize, Debug)]
pub struct Jwt {
    pub sub: String,
    pub email: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
    email: String,
    given_name: Option<String>,
    family_name: Option<String>,
    exp: usize,
    iat: usize,
    iss: String,
    aud: serde_json::Value,
}

impl From<JwtClaims> for Jwt {
    fn from(claims: JwtClaims) -> Self {
        Self {
            sub: claims.sub,
            email: claims.email,
            given_name: claims.given_name,
            family_name: claims.family_name,
        }
    }
}

impl From<Jwt> for authz::Authentication {
    fn from(val: Jwt) -> Self {
        authz::Authentication {
            sub: val.sub,
            email: val.email,
            given_name: val.given_name,
            family_name: val.family_name,
        }
    }
}

async fn verify_keycloak_access_token(token: &str) -> Result<Jwt, ()> {
    let issuer = env::var("OIDC_ISSUER").map_err(|_| ())?;
    let audience = env::var("OIDC_AUDIENCE").map_err(|_| ())?;
    let jwks_url = env::var("OIDC_JWKS_URL").map_err(|_| ())?;

    let header = decode_header(token).map_err(|_| ())?;
    if header.alg != Algorithm::RS256 {
        return Err(());
    }
    let kid = header.kid.ok_or(())?;

    // Fetch the configured realm JWKS. The URL is deployment configuration,
    // never supplied by the request or read from an unverified token.
    let jwks: JwkSet = reqwest::get(&jwks_url)
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())?;

    let jwk = jwks.find(&kid).ok_or(())?;
    let key = DecodingKey::from_jwk(jwk).map_err(|_| ())?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer.as_str()]);
    validation.set_audience(&[audience.as_str()]);
    validation.required_spec_claims = ["exp", "iat", "iss", "sub"]
        .iter()
        .map(|claim| (*claim).to_string())
        .collect();
    validation.validate_exp = true;
    validation.validate_nbf = true;
    validation.leeway = 30;

    let claims = decode::<JwtClaims>(token, &key, &validation).map_err(|_|)?;
    // Access these validated fields explicitly; they are intentionally not
    // accepted from forwarded identity headers.
    let _validated_metadata = (claims.claims.exp, claims.claims.iat, claims.claims.iss, claims.claims.aud);
    if claims.claims.email.trim().is_empty() || claims.claims.sub.trim().is_empty() {
        return Err(());
    }
    Ok(claims.claims.into())
}

impl<S> FromRequestParts<S> for Jwt
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(X_FORWARDED_ACCESS_TOKEN)
            .and_then(|header| header.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Keycloak access token"))?;

        verify_keycloak_access_token(token)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid Keycloak access token"))
    }
}
