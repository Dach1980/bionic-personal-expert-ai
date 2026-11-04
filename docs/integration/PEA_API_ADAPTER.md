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

The Keycloak access token must contain the `pea-api` audience and a `tenant_id` claim. The latter is the selected active tenant for this initial integration. PEA still verifies active membership; the claim alone never grants access.

## Adapter routes

All adapter routes require a verified Keycloak token.

| Bionic route | PEA resource |
|---|---|
| `GET /api/pea/session` | Verified user and tenant session |
| `GET /api/pea/projects` | Current user's authorized project list |
| `GET /api/pea/tenants/{tenant_id}/projects` | Project list |
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

## Agent Runtime integration status

The workspace adapter currently serves PEA data to the Bionic UI. The existing Agent Runtime still uses its standard Bionic tool set; it does **not yet perform semantic/lexical search over PEA SourceFragments or KnowledgeItems**. The shared JWT extractor now validates Keycloak identity for Agent Runtime requests as well, but the access token is not yet propagated into a PEA search tool. Do not describe chat answers as PEA-document-grounded until a tenant/project-scoped PEA search API, a server-side Agent Runtime tool and document-grounded dialogue tests are implemented. Never solve this by giving the browser direct PEA access or by putting service credentials in a prompt.

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

