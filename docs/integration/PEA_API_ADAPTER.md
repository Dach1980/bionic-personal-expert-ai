# Personal Expert AI server-side adapter

The Bionic web server exposes a narrow, server-side adapter under `/api/pea`. The authenticated Bionic workspace is available at `/o/{team_id}/pea`; its Documents, Reviews and Knowledge Base sidebar entries open this workspace. Browser code must call Bionic; it must not call the PEA API directly and must never receive PEA credentials.

## Authentication and trust boundary

- Bionic's `Jwt` extractor accepts only `X-Forwarded-Access-Token` from the configured OIDC gateway.
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

The Keycloak access token must contain the `pea-api` audience and a `tenant_id` claim. The latter is the selected active tenant for this initial integration. PEA still verifies active membership; the claim alone never grants access.

## Adapter routes

All adapter routes require a verified Keycloak token.

| Bionic route | PEA resource |
|---|---|
| `GET /api/pea/tenants/{tenant_id}/projects` | Project list |
| `GET /api/pea/tenants/{tenant_id}/projects/{project_id}/documents` | Project documents |
| `GET /api/pea/tenants/{tenant_id}/projects/{project_id}/documents/{document_id}/versions` | Immutable document versions |
| `GET .../versions/{version_id}/source-fragments` | Source fragments and locators |
| `GET .../versions/{version_id}/artifacts` | Registered rendered-page artifacts |
| `GET .../artifacts/{artifact_id}/content` | Verified artifact bytes |
| `GET .../source-fragments/{fragment_id}/preview` | Source-fragment preview metadata |

The adapter forwards only the upstream status, body and a small allowlist of safe response headers. PEA remains responsible for authorization, storage-key redaction, content integrity and no-store behavior.

## Important deployment note

The example `infra-as-code/docker-compose.yml` is not itself an OIDC gateway. Do not expose the Bionic app port to an untrusted network or assume that setting OIDC environment variables creates a login flow. Use the Stack OIDC component or another trusted gateway that:
1. performs the Keycloak authorization-code flow;
2. validates the session;
3. strips any client-supplied `X-Forwarded-Access-Token` header;
4. injects the actual Keycloak access token into that header over the private app network.

The server-side adapter must remain behind this trust boundary. Never enable the former `DANGER_JWT_OVERRIDE` or trust forwarded user/email headers.

The authenticated workspace page `/o/{team_id}/pea` loads projects, documents, immutable versions, source fragments, rendered page images, reviews, findings, evidence and project knowledge through these same-origin adapter routes. Fragment selection scrolls to the corresponding rendered page when a page locator is available.
