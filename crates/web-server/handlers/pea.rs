use crate::{locale::Locale, CustomError, Jwt};
use axum::{
    body::Body,
    extract::DefaultBodyLimit,
    extract::{Extension, Json, Multipart, Path, Query},
    http::{header, HeaderMap, HeaderName, HeaderValue, Response, StatusCode},
    response::Html,
    routing::{get, post},
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
    let tenant_id = current_user.tenant_id.clone().unwrap_or_default();
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
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "PEA response exceeds adapter limit",
            ));
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

async fn proxy_post(
    path: String,
    user: Jwt,
    payload: serde_json::Value,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "PEA API client could not be initialized",
            )
        })?;
    let upstream = client
        .post(format!("{}{}", base_url.trim_end_matches('/'), path))
        .bearer_auth(user.access_token)
        .json(&payload)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let cache_control = upstream.headers().get(header::CACHE_CONTROL).cloned();
    let body = upstream.bytes().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA API response could not be read",
        )
    })?;
    if body.len() > 32 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "PEA response exceeds adapter limit",
        ));
    }
    let mut response = Response::builder().status(status);
    if let Some(value) = content_type {
        response = response.header(header::CONTENT_TYPE, value);
    }
    if let Some(value) = cache_control {
        response = response.header(header::CACHE_CONTROL, value);
    }
    response.body(Body::from(body)).map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA response could not be forwarded",
        )
    })
}

async fn proxy_post_stream(
    path: String,
    user: Jwt,
    payload: serde_json::Value,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "PEA API client could not be initialized",
            )
        })?;
    let upstream = client
        .post(format!("{}{}", base_url.trim_end_matches('/'), path))
        .bearer_auth(user.access_token)
        .json(&payload)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let cache_control = upstream.headers().get(header::CACHE_CONTROL).cloned();
    let stream = upstream.bytes_stream();
    let mut response = Response::builder().status(status);
    if let Some(value) = content_type {
        response = response.header(header::CONTENT_TYPE, value);
    }
    if let Some(value) = cache_control {
        response = response.header(header::CACHE_CONTROL, value);
    }
    response
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA stream could not be forwarded"))
}

async fn get_session(user: Jwt) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get("/api/v1/session".to_string(), user).await
}

async fn get_current_projects(user: Jwt) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get("/api/v1/projects".to_string(), user).await
}

async fn list_conversations(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/conversations"),
        user,
    )
    .await
}

async fn create_conversation(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_post(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/conversations"),
        user,
        payload,
    )
    .await
}

async fn list_conversation_messages(
    Path((tenant_id, project_id, conversation_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/conversations/{conversation_id}/messages"
        ),
        user,
    )
    .await
}

async fn send_conversation_message(
    Path((tenant_id, project_id, conversation_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_post_stream(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/conversations/{conversation_id}/messages"
        ),
        user,
        payload,
    )
    .await
}

async fn list_projects(
    Path(tenant_id): Path<Uuid>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(format!("/api/v1/tenants/{tenant_id}/projects"), user).await
}

async fn create_document(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_post(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/documents"),
        user,
        payload,
    )
    .await
}

async fn upload_document_version(
    Path((tenant_id, project_id, document_id)): Path<(Uuid, Uuid, Uuid)>,
    user: Jwt,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "Idempotency-Key is required"))?;
    let mut upload: Option<(String, String, Vec<u8>)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid multipart upload"))?
    {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("document").to_string();
            let media_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|_| (StatusCode::BAD_REQUEST, "Uploaded file could not be read"))?;
            if bytes.is_empty() || bytes.len() > 100 * 1024 * 1024 {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "File is empty or exceeds 100 MiB",
                ));
            }
            upload = Some((filename, media_type, bytes.to_vec()));
        }
    }
    let (filename, media_type, bytes) = upload.ok_or((
        StatusCode::BAD_REQUEST,
        "Multipart field 'file' is required",
    ))?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str(&media_type)
        .map_err(|_| {
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Invalid file media type",
            )
        })?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "PEA API client could not be initialized",
            )
        })?;
    let upstream = client
        .post(format!(
            "{}/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(user.access_token)
        .header("Idempotency-Key", idempotency_key)
        .multipart(form)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let body = upstream.bytes().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA API upload response could not be read",
        )
    })?;
    if body.len() > 32 * 1024 * 1024 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "PEA response exceeds adapter limit",
        ));
    }
    let mut response = Response::builder().status(status);
    if let Some(value) = content_type {
        response = response.header(header::CONTENT_TYPE, value);
    }
    response.body(Body::from(body)).map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA upload response could not be forwarded",
        )
    })
}

async fn enqueue_processing(
    Path((tenant_id, project_id, document_id, version_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
    headers: HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or((StatusCode::BAD_REQUEST, "Idempotency-Key is required"))?;
    let base_url = std::env::var("PEA_API_BASE_URL")
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "PEA API is not configured"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|_| {
            (
                StatusCode::BAD_GATEWAY,
                "PEA API client could not be initialized",
            )
        })?;
    let upstream = client
        .post(format!(
            "{}/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/processing-runs",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(user.access_token)
        .header("Idempotency-Key", idempotency_key)
        .json(&payload)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "PEA API is unavailable"))?;
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream.headers().get(header::CONTENT_TYPE).cloned();
    let body = upstream.bytes().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA API response could not be read",
        )
    })?;
    let mut response = Response::builder().status(status);
    if let Some(value) = content_type {
        response = response.header(header::CONTENT_TYPE, value);
    }
    response.body(Body::from(body)).map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "PEA response could not be forwarded",
        )
    })
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

async fn list_processing_runs(
    Path((tenant_id, project_id, document_id, version_id)): Path<(Uuid, Uuid, Uuid, Uuid)>,
    user: Jwt,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_get(
        format!(
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/processing-runs"
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
            "/api/v1/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/source-fragments/{fragment_id}"
        ),
        user,
    )
    .await
}

async fn search_project_sources(
    Path((tenant_id, project_id)): Path<(Uuid, Uuid)>,
    user: Jwt,
    Json(payload): Json<serde_json::Value>,
) -> Result<Response<Body>, (StatusCode, &'static str)> {
    proxy_post(
        format!("/api/v1/tenants/{tenant_id}/projects/{project_id}/search"),
        user,
        payload,
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
        .route("/api/pea/session", get(get_session))
        .route("/api/pea/projects", get(get_current_projects))
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/conversations",
            get(list_conversations).post(create_conversation),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/conversations/{conversation_id}/messages",
            get(list_conversation_messages).post(send_conversation_message),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/search",
            post(search_project_sources),
        )
        .route("/api/pea/tenants/{tenant_id}/projects", get(list_projects))
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents",
            get(list_documents).post(create_document),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions",
            get(list_versions)
                .post(upload_document_version)
                .layer(DefaultBodyLimit::max(100 * 1024 * 1024)),
        )
        .route(
            "/api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions/{version_id}/processing-runs",
            get(list_processing_runs).post(enqueue_processing),
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
