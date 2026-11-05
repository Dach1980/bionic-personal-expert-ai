# Personal Expert AI server-side adapter

The Bionic web server exposes a narrow, server-side adapter under `/api/pea`. The authenticated Bionic workspace is available at `/o/{team_id}/pea`; its Documents, Reviews and Knowledge Base sidebar entries open this workspace. Browser code must call Bionic; it must not call the PEA API directly and must never receive PEA credentials.

## Authentication and trust boundary

The JWT extractor is implemented once in `agent-harness` and re-exported by `web-server`; therefore ordinary Bionic routes, the PEA API adapter and `/completions/{chat_id}` use the same Keycloak RS256/JWKS verification. The legacy `X-Forwarded-User` / `X-Forwarded-Email` fallback and `DANGER_JWT_OVERRIDE` bypass are not accepted. The gateway must still strip client-supplied `X-Forwarded-Access-Token` and inject the actual access token over the private service path.

- Bionic's `Jwt` extractor accepts the trusted Stack-injected `X-Forwarded-Access-Token` header or a standards-based `Authorization: Bearer` token from the public PKCE SPA. The forwarded gateway token takes precedence. Both paths run the same RS256/JWKS, issuer, audience and time-claim validation; identity is never taken from user/email headers.
- The token is verified in the Rust server using RS256, a configured Keycloak JWKS endpoint, exact issuer and audience, and time claims.
- The original verified bearer token is retained only in server memory and forwarded to PEA by the adapter. It is skipped during serialization.
- PEA independently verifies the token and resolves (`iss`, `sub`) through its explicit external-identity table.
- PEA checks the active tenant membership and project membership on every operation.
- The adapter has fixed routes. It is not a general-purpose URL proxy and does not accept arbitrary upstream URLs from the browser.

Required server environment:

- `OIDC_ISSUER`: exact issuer from Keycloak realm discovery;
- `OIDC_AUDIENCE`: `pea-api`;
- `OIDC_JWKS_URL`: configured Keycloak realm JWKS endpoint;
- `PEA_API_BASE_URL`: private/reachable base URL for the PEA API.
- `PEA_WEB_ORIGIN`: one exact public SPA origin (scheme + host only). If empty/unset, cross-origin browser requests are not enabled. CORS allows only Authorization, Content-Type and Accept for GET/POST/OPTIONS; cookies and wildcard origins are disabled.

The Keycloak access token must contain the `pea-api` audience. A `tenant_id` claim is optional and is treated only as a legacy default, never as an authorization grant. The browser first calls `GET /api/pea/tenants`; the adapter forwards it to `GET /api/v1/tenants` without a tenant context. For tenant-scoped routes, Bionic derives `X-Tenant-ID` from the fixed route's `{tenant_id}` path segment and forwards it with the verified caller token. PEA independently verifies the active membership and tenant status on every request.

## Adapter routes

All adapter routes require a verified Keycloak token.

| Bionic route | PEA resource |
|---|---|
| `GET /api/pea/session` | Verified identity before tenant selection |
| `GET /api/pea/tenants` | Active tenant memberships for the verified identity |
| `GET /api/pea/admin/me` | Effective system-admin permissions (PEA RBAC) |
| `GET /api/pea/admin/audit-events` | Global audit events; requires `system.audit.read` |
| `GET /api/pea/admin/tenants` | Platform tenant inventory; requires `system.tenants.read` |
| `GET /api/pea/projects` | Current user's authorized project list |
| `GET /api/pea/tenants/{tenant_id}/projects` | Project list |
| `POST /api/pea/tenants/{tenant_id}/projects/{project_id}/search` | ACL-scoped lexical SourceFragment search (API/proxy only; not yet an Agent Runtime tool) |
| `GET /api/pea/tenants/{tenant_id}/projects/{project_id}/conversations` | Conversation list |
| `POST /api/pea/tenants/{tenant_id}/projects/{project_id}/conversations` | Create conversation |
| `GET/POST .../conversations/{conversation_id}/messages` | Read history / stream local-model SSE response |
| `GET/POST /api/pea/tenants/{tenant_id}/projects/{project_id}/documents` | List / create project documents |
| `GET/POST .../documents/{document_id}/versions` | List / upload immutable versions |
| `GET/POST .../versions/{version_id}/processing-runs` | Processing status / enqueue ingestion |
| `GET /api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions` | Immutable document versions |
| `GET .../versions/{version_id}/source-fragments` | Source fragments and locators |
| `GET .../versions/{version_id}/artifacts` | Registered rendered-page artifacts |
| `GET .../artifacts/{artifact_id}/content` | Verified artifact bytes |
| `GET .../source-fragments/{fragment_id}/preview` | Source-fragment preview metadata |
| `GET .../source-fragments/{fragment_id}` | Source-fragment detail |
| `GET .../reviews` | Project reviews |
| `GET .../reviews/{review_id}/findings` | Review findings |
| `GET .../findings/{finding_id}/evidence` | Finding evidence |
| `GET .../knowledge` | Project knowledge |

The adapter forwards only the upstream status, body and a small allowlist of safe response headers. Multipart uploads are limited to 100 MiB and retain the PEA `Idempotency-Key` contract. PEA remains responsible for authorization, storage-key redaction, content integrity and no-store behavior.

## Agent Runtime implementation design

The detailed source audit, immutable tenant/project context requirements, tool-call continuation lifecycle, PEA search schema, prompt-injection boundary, citation allowlist and acceptance tests are specified in [AGENT_RUNTIME_PEA_SEARCH_DESIGN_2026-11-05.md](AGENT_RUNTIME_PEA_SEARCH_DESIGN_2026-11-05.md). This design is an implementation gate, not a claim that the runtime tool is already active.

## Agent Runtime integration status

The workspace adapter serves PEA data to the Bionic UI. A tenant/project-scoped lexical search API over SourceFragments and its fixed Bionic proxy route are now implemented in source. This is only the retrieval API boundary: the existing Agent Runtime still uses its standard Bionic tool set and does **not yet invoke the PEA search route as a model tool**. The verified caller token is not yet propagated into a PEA search tool invocation. Semantic/vector retrieval, ACL-before-prompt context assembly, provenance citation validation and document-grounded dialogue tests remain outstanding. Do not describe chat answers as PEA-document-grounded until the server-side Agent Runtime tool and end-to-end evidence tests are implemented. Never solve this by giving the browser direct PEA access or by putting service credentials in a prompt.

## Important deployment note

The example `infra-as-code/docker-compose.yml` is not itself an OIDC gateway. Do not expose the Bionic app port to an untrusted network or assume that setting OIDC environment variables creates a login flow. Use the Stack OIDC component or another trusted gateway that:
1. performs the Keycloak authorization-code flow;
2. validates the session;
3. strips any client-supplied `X-Forwarded-Access-Token` header;
4. injects the actual Keycloak access token into that header over the private app network.

The server-side adapter must remain behind this trust boundary. Never enable the former `DANGER_JWT_OVERRIDE` or trust forwarded user/email headers.

The authenticated workspace page `/o/{team_id}/pea` loads projects, documents, immutable versions, source fragments, rendered page images, reviews, findings, evidence and project knowledge through these same-origin adapter routes. Fragment selection scrolls to the corresponding rendered page when a page locator is available.


## Automated Keycloak setup

Use the shared Stack-managed realm; do not create a second Keycloak server. The PEA repository now includes an idempotent bootstrap command that creates the `pea-api` audience client, the `pea-api-context` scope and attaches the scope to Bionic's OIDC client. Follow [`Personal Expert AI Keycloak setup`](https://github.com/Dach1980/Personal_Expert_AI/blob/main/infra/keycloak/README.md) and run `python scripts/configure_keycloak_realm.py` from the PEA repository with the Stack administrator credentials supplied only through environment variables. The script prints the exact issuer and JWKS discovery values. It does not create PEA users or memberships; those remain explicit provisioning steps.

## Keycloak realm configuration

The Stack manifest uses the Stack-managed Keycloak realm `bionic-gpt`; do not provision a second identity server. Stack's OIDC component places oauth2-proxy in front of Bionic and creates the realm for the StackApp. The server validates the forwarded access token independently.

In the Bionic realm, add an Audience mapper to the oauth2-proxy client so the **access token** contains `pea-api` in `aud`. Create a Keycloak client with ID `pea-api` as the resource audience. Add a User Attribute mapper for the `tenant_id` user attribute (String, access token On, ID token Off). Set each user attribute to the corresponding PEA tenant UUID. PEA independently checks the user's active membership.

The Stack profiles configure `OIDC_ISSUER`, `OIDC_AUDIENCE` and the internal Keycloak JWKS endpoint. Verify the exact issuer from the realm's `/.well-known/openid-configuration` before deployment.

For UAT and production, provide the private PEA API URL through a Kubernetes secret in the Bionic namespace:

```bash
kubectl -n bionic-gpt create secret generic pea-api-connection \
  --from-literal=base_url="http://YOUR_PRIVATE_PEA_API_HOST:8000"
```

The secret is projected as `PEA_API_BASE_URL` into the Bionic web server. Do not expose the PEA API or its PostgreSQL database publicly. For local development the Stack dev profile uses `host.docker.internal:8000`; verify this route from the Docker Desktop Kubernetes pod before relying on it.


## Pages SPA integration (2026-11-04)

The public React client uses the Keycloak `pea-web` public client with Authorization Code + PKCE S256. The browser sends the short-lived access token in Authorization to Bionic; the token is held in memory, not persistent browser storage. Bionic validates it and forwards it to PEA, which validates it again. Configure the exact `PEA_WEB_ORIGIN` in Bionic and provision the `pea-web` client plus `pea-api-context` scope using the bootstrap script in the PEA repository.

The Pages document workspace now uses the Adapter to list documents, immutable versions, source fragments, processing runs, rendered artifacts, reviews, findings and evidence. It can create a document, upload a new version, enqueue ingestion with an idempotency key, inspect processing status and preview artifact content. These features are implemented in code; they still require a configured live PEA URL, a configured local model, explicit PEA identity/membership provisioning and a browser E2E pass against the deployed Keycloak/Bionic/PEA stack.



## Tenant selection and administrative read APIs (2026-11-05)

- `GET /api/pea/session` now resolves the verified identity only; `tenantId` is null until the user selects an active membership.
- `GET /api/pea/tenants` returns only the caller's active PEA memberships.
- Tenant-scoped adapter requests derive `X-Tenant-ID` from the adapter's fixed `{tenant_id}` route segment. The browser cannot inject a trusted forwarded-token header or bypass PEA membership checks.
- Bionic's JWT extractor accepts a valid Keycloak access token with no `tenant_id` claim. If the legacy claim exists, it is only a backward-compatible default for legacy routes.
- The adapter proxies `/api/pea/admin/me`, `/api/pea/admin/audit-events`, and `/api/pea/admin/tenants`; PEA performs the permission check and RLS enforcement. No Bionic-side role or browser flag grants administrative access.
- The old `/api/pea/projects` route remains only for legacy tenant-claim clients; the Web client now uses `/api/pea/tenants/{tenant_id}/projects`.

Source implementation and Rust CI are separate acceptance gates. No live Keycloak/Bionic/PEA/PostgreSQL runtime acceptance is implied by these changes.
