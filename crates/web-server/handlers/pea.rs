use crate::{locale::Locale, CustomError, Jwt};
use axum::{
    body::Body,
    extract::{Extension, Path, Query},
    http::{header, HeaderName, HeaderValue, Response, StatusCode},
    response::Html,
    routing::get,
    Router,
};
use axum_extra::routing::RouterExt;
use db::{authz, Pool};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Default)]
struct WorkspaceTab {
    tab: Option<String>,
}

pub async fn index(
    web_pages::routes::pea::Index { team_id }: web_pages::routes::pea::Index,
    Query(query): Query<WorkspaceTab>,
    locale: Locale,
    current_user: Jwt,
    Extension(pool): Extension<Pool>,
) -> Result<Html<String>, CustomError> {
    let tenant_id = current_user.tenant_id.clone();
    let mut client = pool.get().await?;
    let transaction = client.transaction().await?;
    let (rbac, _team_id_num) =
        authz::get_permisisons(&transaction, &current_user.into(), &team_id).await?;
    let tab = match query.tab.as_deref() {
        Some("reviews") => "reviews",
        Some("knowledge") => "knowledge",
        _ => "documents",
    };
    let html = web_pages::pea::page(rbac, team_id, tenant_id, tab, locale.as_str());
    Ok(Html(html))
}

async fn proxy_get(path: String, user: Jwt) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "PEA API client could not be initialized",
            )
        })?;
    let mut upstream = client
        .get(url)
        .bearer_auth(user.access_token)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let headers = upstream.headers().clone();
    const MAX_ADAPTER_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_ADAPTER_RESPONSE_BYTES)
    {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "PEA response exceeds adapter limit",
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = upstream.chunk().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA API response could not be read",
        )
    })? {
        if body.len().saturating_add(chunk.len()) > MAX_ADAPTER_RESPONSE_BYTES {
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "PEA response exceeds adapter limit"));
        }
        body.extend_from_slice(&chunk);
    }

    let mut response = Response::builder().status(status);
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_DISPOSITION,
        header::CACHE_CONTROL,
        header::ETAG,
        HeaderName::from_static("x-content-type-options"),
    ] {
        if let Some(value) = headers.get(&name) {
            if let Ok(value) = HeaderValue::from_bytes(value.as_bytes()) {
                response = response.header(name, value);
            }
        }
    }
    response.body(Body::from(body)).map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA response could not be forwarded",
        )
    })
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
    Path((tenant_id, project_id, document_id, version_id, artifact_id)): Path<(
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
    )>,
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
    Path((tenant_id, project_id, document_id, version_id, fragment_id)): Path<(
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
    )>,
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


async fn get_document_version(
    Path((tenant_id, project_id, document_id, version_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}"
        ),
        user,
    )
    .await
}

async fn get_source_fragment(
    Path((tenant_id, project_id, document_id, version_id, fragment_id)): Path<(Uuid, Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}"
        ),
        user,
    )
    .await
}

async fn list_knowledge(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/knowledge"),
        user,
    )
    .await
}


async fn list_reviews(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/reviews"),
        user,
    )
    .await
}

async fn list_findings(
    Path((tenant_id, project_id, review_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/reviews/{review_id}/findings"),
        user,
    )
    .await
}

async fn list_evidence(
    Path((tenant_id, project_id, finding_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/findings/{finding_id}/evidence"),
        user,
    )
    .await
}

pub fn routes() -> Router {
    Router::new()
        .typed_get(index)
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
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}",
            get(get_document_version),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}",
            get(get_source_fragment),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/knowledge",
            get(list_knowledge),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/reviews",
            get(list_reviews),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/reviews/{review_id}/findings",
            get(list_findings),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/findings/{finding_id}/evidence",
            get(list_evidence),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}/preview",
            get(source_fragment_preview),
        )
}
