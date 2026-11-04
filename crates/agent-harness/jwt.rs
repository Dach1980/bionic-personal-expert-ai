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

    let signature = URL_SAFE_NO_PAD.decode(parts[2]).map_err(|_| ())?;
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    verify_rs256_signature(&signing_input, &signature, jwk)?;

    let payload = URL_SAFE_NO_PAD.decode(parts[1]).map_err(|_| ())?;
    let claims: JwtClaims = serde_json::from_slice(&payload).map_err(|_| ())?;
    validate_claims(&claims, &issuer, &audience, unix_time()?)?;

    Ok(Jwt {
        sub: claims.sub,
        email: claims.email,
        tenant_id: claims.tenant_id,
        given_name: claims.given_name,
        family_name: claims.family_name,
        access_token: token.to_string(),
    })
}

fn verify_rs256_signature(signing_input: &str, signature: &[u8], jwk: &Jwk) -> Result<(), ()> {
    let modulus = URL_SAFE_NO_PAD
        .decode(jwk.n.as_deref().ok_or(())?)
        .map_err(|_| ())?;
    let exponent = URL_SAFE_NO_PAD
        .decode(jwk.e.as_deref().ok_or(())?)
        .map_err(|_| ())?;
    RsaPublicKeyComponents {
        n: &modulus,
        e: &exponent,
    }
    .verify(
        &RSA_PKCS1_2048_8192_SHA256,
        signing_input.as_bytes(),
        signature,
    )
    .map_err(|_| ())
}

fn validate_claims(claims: &JwtClaims, issuer: &str, audience: &str, now: u64) -> Result<(), ()> {
    let leeway = 30_u64;
    if claims.iss != issuer
        || !claims.aud.contains(audience)
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
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::engine::general_purpose::STANDARD;

    const TEST_MODULUS: &str = "wKEAZDUCOy_lY-ZcMV6x85RbIgy7I9AGsrIk1p4easE2TomiiMQgcDW1sGcBxl0ziSIQAWaOuvIEZRZDOlYD6rsZlBDDh3pJUH1JlOhNfhV3XDwyCw0XEjbVVSaWp8Qkfj65n6x6x5hveuVye1LzYmpyDQJO7fTR4qMGZNzE3xDTVGxqNA5hkOC7A1Keub0BeQb52Y8A0O2YhMw5IsT_gG9N4N7wtpaXqrfkGQQr4p4X8_TDupdb2nI0wzR2OKORHMDXQ2PiUx_r7v-2wl9DglbYdeD1Fl19Gn7UZ3m5vAjo0uvJAWjpz8C5aqi5MSl--_X764Xd3E5rcKtzdODqMQ";

    fn test_jwk() -> Jwk {
        Jwk {
            kid: "test-kid".to_string(),
            kty: "RSA".to_string(),
            use_: Some("sig".to_string()),
            alg: Some("RS256".to_string()),
            n: Some(TEST_MODULUS.to_string()),
            e: Some("AQAB".to_string()),
        }
    }

    fn test_claims(now: u64) -> JwtClaims {
        JwtClaims {
            sub: "keycloak-subject".to_string(),
            email: "user@example.test".to_string(),
            tenant_id: uuid::Uuid::new_v4().to_string(),
            given_name: None,
            family_name: None,
            exp: now + 300,
            iat: now,
            iss: "https://sso.example.test/realms/bionic-gpt".to_string(),
            aud: Audience::Multiple(vec!["account".to_string(), "pea-api".to_string()]),
            nbf: None,
        }
    }

    #[test]
    fn rs256_signature_accepts_the_matching_jwk_and_rejects_tampering() {
        const TEST_SIGNATURE: &str = "UXwAXEufhWY8fP4LP+2sdZyA8K6Lo9HD6Q5Vc0anic2kiP8a3SNLtgnpKMaRXWcS8Rkb9BOVYuXByGKxk9lxK6a61Nsd0DizYVi6L5pXH01SKLM9n4u+2wR0jMLJ3q+oRWNJHyBinMOG97+01XQU0HiWpZnqXPXmT5Yl56+BBLkwsIx31Ncb+q4wQDs9HAvff9BJQ+iPTYDKc6zI7VaGVU8UWUvuo6eProTeZgNxYaou9gP02HT60MgyNVj6RK4DT2+ukAcixUCVw8F9OvXGGYN8B4Y2t1OLal3ysWDF1gasgAU5yXsgNWgrvPmHg9rOk9cESPsX/EjW3/Bn3o3+Ig==";
        let signing_input = "eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3Qta2lkIn0.eyJzdWIiOiJ0ZXN0In0";
        let mut signature = STANDARD.decode(TEST_SIGNATURE).unwrap();

        let jwk = test_jwk();
        assert!(verify_rs256_signature(signing_input, &signature, &jwk).is_ok());

        signature[0] ^= 0x01;
        assert!(verify_rs256_signature(signing_input, &signature, &jwk).is_err());
    }

    #[test]
    fn claims_require_the_configured_issuer_audience_and_valid_time_window() {
        let now = 1_800_000_000;
        let claims = test_claims(now);
        assert!(validate_claims(
            &claims,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_ok());

        let mut wrong_issuer = test_claims(now);
        wrong_issuer.iss = "https://attacker.example/realm".to_string();
        assert!(validate_claims(
            &wrong_issuer,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());

        let mut wrong_audience = test_claims(now);
        wrong_audience.aud = Audience::Single("another-api".to_string());
        assert!(validate_claims(
            &wrong_audience,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());

        let mut expired = test_claims(now);
        expired.exp = now - 60;
        assert!(validate_claims(
            &expired,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());

        let mut malformed_tenant = test_claims(now);
        malformed_tenant.tenant_id = "not-a-uuid".to_string();
        assert!(validate_claims(
            &malformed_tenant,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());
    }

    #[test]
    fn claims_reject_future_iat_and_not_before() {
        let now = 1_800_000_000;
        let mut future_iat = test_claims(now);
        future_iat.iat = now + 60;
        assert!(validate_claims(
            &future_iat,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());

        let mut future_nbf = test_claims(now);
        future_nbf.nbf = Some(now + 60);
        assert!(validate_claims(
            &future_nbf,
            "https://sso.example.test/realms/bionic-gpt",
            "pea-api",
            now
        )
        .is_err());
    }

    #[tokio::test]
    async fn rejects_untrusted_forwarded_identity_headers_without_access_token() {
        use axum::{body::Body, http::Request};

        let request = Request::builder()
            .header("X-Forwarded-User", "forged-subject")
            .header("X-Forwarded-Email", "forged@example.test")
            .body(Body::empty())
            .unwrap();
        let (mut parts, _) = request.into_parts();

        let result = Jwt::from_request_parts(&mut parts, &()).await;
        assert!(matches!(result, Err((StatusCode::UNAUTHORIZED, _))));
    }
}
