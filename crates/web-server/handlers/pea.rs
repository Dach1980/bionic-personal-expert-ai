use axum::{
    body::Body,
    extract::Path,
    http::{header, HeaderValue, Response, StatusCode},
    routing::get,
    Router,
};
use crate::Jwt;
use uuid::Uuid;

async fn proxy_get(path: String, user: Jwt) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);

    let upstream = reqwest::Client::new()
        .get(url)
        .bearer_auth(user.access_token)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;

    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let headers = upstream.headers().clone();
    let body = upstream
        .bytes()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API response could not be read"))?;

    let mut response = Response::builder().status(status);
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_DISPOSITION,
        header::CACHE_CONTROL,
        header::ETAG,
        header::X_CONTENT_TYPE_OPTIONS,
    ] {
        if let Some(value) = headers.get(&name) {
            if let Ok(value) = HeaderValue::from_bytes(value.as_bytes()) {
                response = response.header(name, value);
            }
        }
    }
    response
        .body(Body::from(body.to_vec()))
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA response could not be forwarded"))
}

async fn list_projects(
    Path(tenant_id): Path<Uuid>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(format!("/api/v1/tenants/{tenant_id}/projects"), user).await
}

async fn list_documents(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/documents"),
        user,
    )
    .await
}

async fn list_versions(
    Path((tenant_id, project_id, document_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions"
        ),
        user,
    )
    .await
}

async fn list_source_fragments(
    Path((tenant_id, project_id, document_id, version_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments"
        ),
        user,
    )
    .await
}

async fn list_artifacts(
    Path((tenant_id, project_id, document_id, version_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/artifacts"
        ),
        user,
    )
    .await
}

async fn artifact_content(
    Path((tenant_id, project_id, document_id, version_id, artifact_id)): Path<(Uuid, Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/artifacts/{artifact_id}/content"
        ),
        user,
    )
    .await
}

async fn source_fragment_preview(
    Path((tenant_id, project_id, document_id, version_id, fragment_id)): Path<(Uuid, Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}/preview"
        ),
        user,
    )
    .await
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/pea/tenants/{tenant_id}/projects", get(list_projects))
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents",
            get(list_documents),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions",
            get(list_versions),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments",
            get(list_source_fragments),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/artifacts",
            get(list_artifacts),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/artifacts/{artifact_id}/content",
            get(artifact_content),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}/preview",
            get(source_fragment_preview),
        )
}
