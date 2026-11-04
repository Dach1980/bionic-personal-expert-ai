# Personal Expert AI — Bionic fork adaptation

## Status

The repository is the user-owned fork of Bionic:

- Repository: https://github.com/Dach1980/bionic-personal-expert-ai
- Working branch: `main`
- Upstream: https://github.com/bionic-gpt/bionic-gpt
- Adaptation strategy: preserve Bionic's agent/runtime and replace the product identity and selected UI surfaces incrementally.

This document records verified UI entry points and the first branding changes. It does not claim that the application has been built or runtime-tested.

## Verified UI structure

Bionic's application UI is server-rendered with Dioxus and is split into Rust crates:

- `crates/web-pages` — page components and application layout.
- `crates/web-server` — Axum routes/handlers and HTML response orchestration.
- `crates/web-assets` — static SVG, CSS and TypeScript assets, exposed through generated `assets::files` constants.

The crate README confirms that each route's page is implemented under `crates/web-pages/<feature>/page.rs`; `routes.rs` defines typed paths, while web-server handlers load data and invoke page components.

### Main UI adaptation points

| Concern | Verified file(s) | Responsibility |
|---|---|---|
| Global shell, document head, favicon, sidebar and footer | `crates/web-pages/app_layout/base.rs` | Shared HTML shell and responsive layout |
| Product name, logo selection, sidebar header and footer composition | `crates/web-pages/app_layout/mod.rs` | Main/admin layout composition |
| Main navigation | `crates/web-pages/app_layout/sidebar.rs`, `menu.rs` | Work and Context navigation groups |
| Project navigation | `crates/web-pages/app_layout/project_sidebar.rs` | Project list and selected project state |
| Chat page composition | `crates/web-pages/console/page.rs`, `console/layout.rs` | Chat route and main conversation layout |
| Chat input, attachments and submit controls | `crates/web-pages/console/prompt_form.rs` | Message form and user input |
| Streaming conversation | `crates/web-pages/console/console_stream.rs`, `console/conversation.rs`, `console/assistant_response.rs` | Conversation stream and rendered responses |
| Theme and global styling | `crates/web-assets/input.css` | Tailwind/daisyUI theme and shared CSS |
| Static asset registration | `crates/web-assets/build.rs`, `crates/web-assets/lib.rs` | Generated Rust references to images and built assets |
| Static asset delivery | `crates/web-server/handlers/static_files.rs` | Serves assets under `/static/*` |

## Branding changes already committed

1. Added the supplied logo as `crates/web-assets/images/personal-expert-ai-logo.svg`.
   - Source: `https://github.com/Dach1980/cherkashin_ai_site/blob/main/assets/logo.svg`
   - The source SVG contains an embedded raster payload and is approximately 2.43 MB. Keep this in mind for later asset optimization; do not silently substitute a different logo.
2. Updated `crates/web-pages/app_layout/mod.rs`:
   - Application brand name is now `Personal Expert AI`.
   - The app logo reference uses the generated asset constant `personal_expert_ai_logo_svg`.
   - The same logo source is supplied to the shared layout as the favicon.
3. Updated `crates/web-pages/app_layout/base.rs`:
   - Browser document titles are prefixed with `Personal Expert AI —`.
   - Added a compact Personal Expert AI logo/name signature to the bottom of the application sidebar as its footer branding.

These changes are source-level commits only. Compilation, browser rendering, responsive layout and favicon behavior still need runtime verification.

## Navigation direction

The current Bionic navigation is organized around Work, Context and project selection. The intended Personal Expert AI information architecture is:

- Projects
- Documents
- Dialogues
- Reviews
- Knowledge Base
- Skills
- Settings

Do not expose navigation destinations until their backend/API contracts exist. In particular, Reviews and evidence views must be backed by Personal Expert AI domain APIs, not by fabricated Bionic dataset records.

## Architecture boundaries

- Personal Expert AI remains the system of record for tenants, projects, documents, document versions, extracted fragments, provenance, engineering reviews and evidence.
- Bionic remains the presentation/agent runtime layer.
- Bionic must not connect directly to Personal Expert AI's PostgreSQL database or receive database credentials.
- Integration must use authenticated, tenant-scoped backend APIs. The backend derives the principal and permitted tenant/project scope; model-supplied identifiers are not authorization.
- Start with read-only integration tools. Add write operations only with explicit user confirmation and server-side authorization.
- Keep local inference/local processing as the deployment target. No silent cloud-model or external-processing fallback.

## Next implementation sequence

1. Finish the UI shell adaptation and verify logo, favicon, footer, responsive sidebar and browser title in a local build.
2. Rework the sidebar labels/grouping without changing route handlers or authorization checks.
3. Adapt the empty chat state, conversation header and evidence presentation while preserving streaming and cancellation behavior.
4. Add a Personal Expert API adapter as a separate integration boundary.
5. Implement and test the missing backend read contracts for document versions, SourceFragment/provenance and evidence retrieval before exposing document-grounded Q&A.
6. Run compile, unit, integration and browser smoke tests. Do not treat static source inspection as runtime acceptance.

## Explicitly out of scope for the first UI pass

Do not rewrite the agent loop, tool dispatcher, sandbox, authentication, database schema, RAG ingestion or model-provider implementation as part of visual branding. These areas require separate design and regression testing.


## Current UI implementation — 2026-11-03

The first navigation and empty-chat adaptation pass is committed on `main`.

### Navigation mapping

| Personal Expert AI section | Current implementation | Availability |
|---|---|---|
| Projects | PEA project list loaded through the authenticated server adapter | Active; PEA checks tenant and project membership |
| Documents | PEA document list, immutable versions, SourceFragments and rendered-page artifacts | Active; source bytes are served only through authenticated PEA artifact routes |
| Dialogues | Existing Bionic console route and conversation history | Active; uses Bionic Agent Runtime and chat permissions |
| Reviews | PEA Review list with Finding and Evidence drill-down | Active for reading; domain write APIs exist in PEA, but the Bionic adapter is deliberately read-only at this stage |
| Knowledge Base | PEA project KnowledgeItem listing | Active; PEA checks project membership |
| Skills | Existing Bionic Skills route | Active, protected by existing chat permission |
| Settings | Existing Bionic administration entry point | Visible only to users who already have the corresponding administration permission |

The old Bionic Context group has been replaced with a Personal Expert AI Workspace group. Documents, Reviews and Knowledge Base open the authenticated PEA workspace at `/o/{team_id}/pea` with a tab selection. They are backed by live PEA API responses, not Bionic Datasets or fabricated records.

### Empty chat

The empty state introduces Personal Expert AI and describes project exploration, evidence-based documentation review and source traceability. The existing prompt form, submit action, conversation stream, stop/cancel control, conversation history and MCP app panel remain intact. The latest completed baseline CI built the Rust web server and browser assets successfully; a new JWT-focused Rust test suite is being validated separately. Live browser testing against a running Keycloak/PEA stack is still outstanding.

### Document and evidence cards — binding contract

Cards must be rendered from Personal Expert AI API responses, not Bionic Dataset/Document records. The UI adapter must map the following fields:

**Document card**
- `document_id`, `project_id`, `title`, `file_name`, `media_type`
- current `document_version_id`, version label/number and processing status
- page/sheet count only when supplied by extraction metadata
- a UI-owned document-view URL carrying the document and selected version identifiers

**Evidence card**
- `fragment_id`, `document_version_id`, document title and version
- safe excerpt returned by the backend
- typed locator: page, bounding box, sheet, table, row/cell or text anchor
- a UI-owned deep link carrying the fragment and immutable version identifiers

**Navigation behavior**
1. A page locator opens the exact page in the selected document version and applies the available bounding-box highlight.
2. A spreadsheet locator opens the named sheet and selected table/range or cell.
3. A text locator opens the fragment in its source context and highlights the matching excerpt.
4. Every viewer request is resolved through the authenticated Personal Expert AI API. The browser never receives a storage path or direct object-store credential.
5. If a locator cannot be resolved, show the excerpt and source metadata with an explicit “location unavailable” state; never silently redirect to page 1.

The API specification currently defines the relevant resource families under `/api/v1/projects/{project_id}/documents`, `/api/v1/documents/{document_id}/versions`, `/api/v1/source-fragments/{fragment_id}`, `/api/v1/source-fragments/{fragment_id}/preview`, `/api/v1/projects/{project_id}/knowledge` and `/api/v1/findings/{finding_id}/evidence`. These are target contracts: the adapter must check actual backend implementation and OpenAPI before enabling the navigation items.

### Next implementation gates

1. Verify current backend endpoints and publish/validate OpenAPI for project/document/version/fragment/knowledge/review/evidence reads.
2. Implement a typed Personal Expert API adapter in Bionic, using the existing authenticated user session and server-side tenant/project scope. Never forward browser-supplied tenant scope as authorization.
3. Add document list/detail and source viewer routes, then connect the document/evidence cards to those routes.
4. Add contract tests for access denial across projects/tenants, immutable version resolution, locator variants and missing source files.
5. Run `cargo fmt --check`, `cargo test -p web-pages` (or the repository's documented equivalent), full build, and browser tests for login, chat submit, SSE streaming, stop/cancel, history, and source navigation.

Do not enable Documents, Reviews or Knowledge Base merely because the navigation labels exist. Enable each only after its API contract, authorization and end-to-end source navigation tests pass.


## Integration progress — 2026-11-03

### Personal Expert AI backend read API

Implemented in the Personal Expert AI repository:
- tenant-scoped document version list and detail;
- version-scoped SourceFragment list and detail;
- version-scoped rendered-page artifact metadata;
- authenticated rendered-page image streaming with checksum/size verification;
- project-scoped KnowledgeItem listing.

Storage keys and absolute filesystem paths are removed recursively from browser-facing locators. Rendered-page bytes are exposed only by opaque artifact ID after checking tenant, project, document, immutable version and project membership.

Review, Finding and Evidence are now implemented as PEA domain entities with Alembic migration `0009_reviews_findings_evidence`, create/read/status APIs, project-role authorization and audit events. Migration `0010_review_domain_rls` adds PostgreSQL tenant RLS for these tables. Bionic currently exposes their read endpoints through the server adapter; write APIs remain available directly in PEA and can be added to the adapter after the UI mutation/CSRF contract is designed.

### Identity boundary

PEA and Bionic now validate Keycloak RS256/JWKS access tokens against configured issuer and audience. PEA maps the verified `(issuer, subject)` pair through `external_identities`, then checks active PEA user, tenant membership and project membership. Identity provisioning is explicit; it does not create tenants or grants. Bionic's fixed server-side adapter forwards the original verified access token to PEA; the browser never receives PEA database credentials or service secrets. The remaining security gate is runtime verification that the Stack OIDC gateway strips client-supplied `X-Forwarded-Access-Token` and injects the actual token.

### GitHub Pages visual preview

A static, synthetic UI preview has been added at `demo/index.html`, with a GitHub Actions Pages deployment workflow at `.github/workflows/pages-demo.yml`. It demonstrates navigation, project/document cards, a chat empty state, source evidence cards, and illustrative PDF/table deep-link interactions.

This preview is intentionally disconnected from FastAPI, PostgreSQL, JWT authentication, Bionic server routes and Agent Runtime. All displayed records and sample assistant responses are fictional. It is a design review surface, not a deployed or functional Personal Expert AI instance.

To enable deployment, set repository Settings → Pages → Build and deployment → Source to **GitHub Actions**. The preview URL will be `https://dach1980.github.io/bionic-personal-expert-ai/` once the Pages workflow succeeds.


### CI evidence after backend API additions

The Personal Expert AI PostgreSQL 16 workflow completed successfully for the source-fragment preview route and OpenAPI registration commit:
- Alembic upgrade, downgrade and re-upgrade: passed
- ORM/database schema parity: passed
- PostgreSQL integration and full pytest suite: 86 passed
- Ruff lint: passed

This validates the PEA backend and Rust/UI build independently. It does not yet validate a running Bionic-to-PEA session through a live Keycloak realm, nor the complete browser login → dialogue → Agent Runtime → source evidence journey. Those runtime tests require the local Stack OIDC gateway, Keycloak realm configuration, PEA API and Bionic to be running together.
