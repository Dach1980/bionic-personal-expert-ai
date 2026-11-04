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

fn validate_claims(
    claims: &JwtClaims,
    issuer: &str,
    audience: &str,
    now: u64,
) -> Result<(), ()> {
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
    use ring::{
        rand::SystemRandom,
        signature::{RsaKeyPair, RSA_PKCS1_SHA256},
    };

    const TEST_PRIVATE_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDAoQBkNQI7L+Vj5lwxXrHzlFsiDLsj0AaysiTWnh5qwTZOiaKIxCBwNbWwZwHGXTOJIhABZo668gRlFkM6VgPquxmUEMOHeklQfUmU6E1+FXdcPDILDRcSNtVVJpanxCR+PrmfrHrHmG965XJ7UvNianINAk7t9NHiowZk3MTfENNUbGo0DmGQ4LsDUp65vQF5BvnZjwDQ7ZiEzDkixP+Ab03g3vC2lpeqt+QZBCvinhfz9MO6l1vacjTDNHY4o5EcwNdDY+JTH+vu/7bCX0OCVth14PUWXX0aftRnebm8COjS68kBaOnPwLlqqLkxKX779fvrhd3cTmtwq3N04OoxAgMBAAECggEADvUxk5ZuKuxoMp6zr/27cb+mZ/l3GqvPqVad97BlikKgle6oGVipcC0rg5wsDhw0oURs2777/FZIsxTMeZ4uRa7rqIy4si9zu/VOnmp/gffmwdnGxQbqMLbjT+5kKnjXoJn8TgLVseQLXA6vGhZx4cWEP7BbMq0dEy03lONzwiJoxC684K9AMrYcGlxTJ0GBCBMgVfJVG7MpT8h4445EBVzqVsRX89v/dW0bJAh+zFnUnQhPXGPPs5h1GtwsPw3vzONnAqCYwBcMrrKt+r9ln8DKKUSqhXvhxUsXk+6wwuKnILaOVd9jlgDlMz2S+eKWkT5vJpleNYtocLjYvKUb5QKBgQDuJQ+ZmZD3Yg/zReajrvrN8zLKhuUJmkRuSbTJfosJIa6oE6tZATDvEBTl/CjtyNjKkwdoXipzXUjydGZ2teUr2KwaiD7YD40fIY2Q2iHXMWeSCJQ9lzG3PRUul9m/eKiWd0S/2y3fshdxCoum0lAU3Fkhfk8T3rbs1jUIdDFCDwKBgQDPEk+7jg7mygo8xmeg6tUZ8F2yabhZm47nyAFrUFxkWsgblJ/2eOIYnGtTxkAPukLQYO5X2tUItrOclq0LEcAc/LBIuiiK27RdVxx+397U8S8qjihBrQdKDndwCMHl9iNNNWL4oLxzO5F6eNMQFW0vx1P+CmPFXfG94UboSWJPvwKBgCsv42VamRST3wqiGZKDkA47VXGvBug35sMNcNC9tx8f0Zm/f2pzsdkY1IFkulNtAguyoDxIH3CucSUvAMn0cKwD7iodnS/3mgMURaUIOezZk8vC7IdC3Qw68pVxilaT3hbzaC9W4znXBTNF00+cglNWJn/RdSvITuoiapmu4KP3AoGAfIU0cULB70L72QQuePP+ZfTHQ2yTsT78NqFtSQ0wIiD9SKidcOfMfqRBXkVPbKXDW++OJEKx7OWBKN7q/EcWUdgaYgGgM7Bey+qAdkNFiqPmix6lUqksQvpkFHkPsKvNGrbJ510xNdJSb2WM9pUL7bld5tx0ltEhd9+qdjD6750CgYBLCpVhy8E2PKtOW8aQ1uQ+oMS5PWuFPk97tqz7Bq8ESzWU7273s1CC7EB/0o7JYFTUFAU6uKb7RL+stm/x+r3l3qQFwWhbg65cWSYUXR04kXOpgghXApIWBgPdRJOXSsAoQ9CIoyjsAbPhw4fV0JsTNPuG5TSO/x951Q5NkW2UFA==";
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
        let key_bytes = STANDARD.decode(TEST_PRIVATE_KEY).unwrap();
        let key_pair = RsaKeyPair::from_pkcs8(&key_bytes).unwrap();
        let signing_input = "eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3Qta2lkIn0.eyJzdWIiOiJ0ZXN0In0";
        let mut signature = vec![0; key_pair.public_modulus_len()];
        key_pair
            .sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), signing_input.as_bytes(), &mut signature)
            .unwrap();

        let jwk = test_jwk();
        assert!(verify_rs256_signature(signing_input, &signature, &jwk).is_ok());

        signature[0] ^= 0x01;
        assert!(verify_rs256_signature(signing_input, &signature, &jwk).is_err());
    }

    #[test]
    fn claims_require_the_configured_issuer_audience_and_valid_time_window() {
        let now = 1_800_000_000;
        let claims = test_claims(now);
        assert!(validate_claims(&claims, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_ok());

        let mut wrong_issuer = test_claims(now);
        wrong_issuer.iss = "https://attacker.example/realm".to_string();
        assert!(validate_claims(&wrong_issuer, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());

        let mut wrong_audience = test_claims(now);
        wrong_audience.aud = Audience::Single("another-api".to_string());
        assert!(validate_claims(&wrong_audience, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());

        let mut expired = test_claims(now);
        expired.exp = now - 60;
        assert!(validate_claims(&expired, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());

        let mut malformed_tenant = test_claims(now);
        malformed_tenant.tenant_id = "not-a-uuid".to_string();
        assert!(validate_claims(&malformed_tenant, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());
    }

    #[test]
    fn claims_reject_future_iat_and_not_before() {
        let now = 1_800_000_000;
        let mut future_iat = test_claims(now);
        future_iat.iat = now + 60;
        assert!(validate_claims(&future_iat, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());

        let mut future_nbf = test_claims(now);
        future_nbf.nbf = Some(now + 60);
        assert!(validate_claims(&future_nbf, "https://sso.example.test/realms/bionic-gpt", "pea-api", now).is_err());
    }
}
