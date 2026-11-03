use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use db::authz;
use ring::signature::{RsaPublicKeyComponents, RSA_PKCS1_2048_8192_SHA256};
use serde::{Deserialize, Serialize};
use std::{
    env,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const X_FORWARDED_ACCESS_TOKEN: &str = "X-Forwarded-Access-Token";
const JWKS_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Serialize, Deserialize)]
pub struct Jwt {
    pub sub: String,
    pub email: String,
    pub tenant_id: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    #[serde(skip)]
    pub access_token: String,
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

#[derive(Debug, Deserialize)]
struct JwtHeader {
    alg: String,
    kid: String,
}

#[derive(Clone, Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    #[serde(rename = "use")]
    use_: Option<String>,
    alg: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

impl Audience {
    fn contains(&self, expected: &str) -> bool {
        match self {
            Self::Single(value) => value == expected,
            Self::Multiple(values) => values.iter().any(|value| value == expected),
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    sub: String,
    email: String,
    tenant_id: String,
    given_name: Option<String>,
    family_name: Option<String>,
    exp: u64,
    iat: u64,
    iss: String,
    aud: Audience,
    nbf: Option<u64>,
}

static JWKS_CACHE: OnceLock<RwLock<Option<(Instant, JwkSet)>>> = OnceLock::new();

async fn load_jwks(url: &str, force_refresh: bool) -> Result<JwkSet, ()> {
    let cache = JWKS_CACHE.get_or_init(|| RwLock::new(None));
    if !force_refresh {
        let cached = cache.read().await;
        if let Some((loaded_at, jwks)) = cached.as_ref() {
            if loaded_at.elapsed() < JWKS_CACHE_TTL {
                return Ok(jwks.clone());
            }
        }
    }

    // The URL is fixed deployment configuration, never supplied by the request.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| ())?;
    let jwks = client
        .get(url)
        .send()
        .await
        .map_err(|_| ())?
        .error_for_status()
        .map_err(|_| ())?
        .json::<JwkSet>()
        .await
        .map_err(|_| ())?;

    let mut cached = cache.write().await;
    *cached = Some((Instant::now(), jwks.clone()));
    Ok(jwks)
}

fn unix_time() -> Result<u64, ()> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ())?
        .as_secs())
}

async fn verify_keycloak_access_token(token: &str) -> Result<Jwt, ()> {
    let issuer = env::var("OIDC_ISSUER").map_err(|_| ())?;
    let audience = env::var("OIDC_AUDIENCE").map_err(|_| ())?;
    let jwks_url = env::var("OIDC_JWKS_URL").map_err(|_| ())?;

    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(());
    }

    let header_bytes = URL_SAFE_NO_PAD.decode(parts[0]).map_err(|_| ())?;
    let header: JwtHeader = serde_json::from_slice(&header_bytes).map_err(|_| ())?;
    if header.alg != "RS256" || header.kid.trim().is_empty() {
        return Err(());
    }

    let mut jwks = load_jwks(&jwks_url, false).await?;
    let mut jwk = jwks.keys.iter().find(|key| {
        key.kid == header.kid
            && key.kty == "RSA"
            && key.use_.as_deref().is_none_or(|usage| usage == "sig")
            && key
                .alg
                .as_deref()
                .is_none_or(|algorithm| algorithm == "RS256")
    });
    if jwk.is_none() {
        jwks = load_jwks(&jwks_url, true).await?;
        jwk = jwks.keys.iter().find(|key| {
            key.kid == header.kid
                && key.kty == "RSA"
                && key.use_.as_deref().is_none_or(|usage| usage == "sig")
                && key
                    .alg
                    .as_deref()
                    .is_none_or(|algorithm| algorithm == "RS256")
        });
    }
    let jwk = jwk.ok_or(())?;

    let modulus = URL_SAFE_NO_PAD
        .decode(jwk.n.as_deref().ok_or(())?)
        .map_err(|_| ())?;
    let exponent = URL_SAFE_NO_PAD
        .decode(jwk.e.as_deref().ok_or(())?)
        .map_err(|_| ())?;
    let signature = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| ())?;
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    RsaPublicKeyComponents {
        n: &modulus,
        e: &exponent,
    }
    .verify(
        &RSA_PKCS1_2048_8192_SHA256,
        signing_input.as_bytes(),
        &signature,
    )
    .map_err(|_| ())?;

    let payload = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| ())?;
    let claims: JwtClaims = serde_json::from_slice(&payload).map_err(|_| ())?;
    let now = unix_time()?;
    let leeway = 30_u64;

    if claims.iss != issuer
        || !claims.aud.contains(&audience)
        || claims.sub.trim().is_empty()
        || claims.email.trim().is_empty()
        || uuid::Uuid::parse_str(&claims.tenant_id).is_err()
        || claims.exp <= now.saturating_sub(leeway)
        || claims.iat > now.saturating_add(leeway)
        || claims.exp <= claims.iat
        || claims
            .nbf
            .is_some_and(|nbf| nbf > now.saturating_add(leeway))
    {
        return Err(());
    }

    Ok(Jwt {
        sub: claims.sub,
        email: claims.email,
        tenant_id: claims.tenant_id,
        given_name: claims.given_name,
        family_name: claims.family_name,
        access_token: token.to_string(),
    })
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
