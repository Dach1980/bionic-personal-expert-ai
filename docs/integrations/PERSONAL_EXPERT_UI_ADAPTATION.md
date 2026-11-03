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
| Projects | Existing Bionic project list and project routes | Active, protected by existing project RBAC |
| Documents | Reserved navigation item | Disabled until the Personal Expert API adapter and document list view are connected |
| Dialogues | Existing console route for a new dialogue and Bionic history route | Active, protected by existing chat/history RBAC |
| Reviews | Reserved navigation item | Disabled until review and finding APIs are connected |
| Knowledge Base | Reserved navigation item | Disabled until Personal Expert AI knowledge/search APIs are connected |
| Skills | Existing Bionic Skills route | Active, protected by existing chat permission |
| Settings | Existing Bionic administration entry point | Visible only to users who already have the corresponding administration permission |

The old Bionic Context group has been replaced with a Personal Expert AI Workspace group. Projects are displayed first. The reserved entries have no destination and are marked disabled; they must not be linked to Bionic Datasets or other semantically different pages.

### Empty chat

The empty state now introduces Personal Expert AI and describes project exploration, evidence-based documentation review and source traceability. The existing prompt form, submit action, conversation stream, stop/cancel control, conversation history and MCP app panel are left intact. This is a source-level UI change; a successful Rust build and browser smoke test have not yet been recorded.

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

Not yet implemented in the backend: Review, Finding and explicit Evidence domain resources. The current database schema does not contain Review/Finding tables. Do not connect the Reviews navigation to a placeholder or unrelated Bionic audit trail.

### Identity boundary

The current Personal Expert AI API trusts HS256 JWTs from a configured issuer/audience and resolves `sub` to a Personal Expert AI User UUID. Bionic has its own existing user/session model. No verified mapping between those identities has been implemented. A Bionic server-side adapter must not fabricate JWTs, accept tenant IDs as proof of access, or expose a service credential to the browser. The next integration task is a documented trusted identity bridge (or an explicitly provisioned identity mapping) with negative tests for cross-tenant and cross-project access.

### GitHub Pages visual preview

A static, synthetic UI preview has been added at `demo/index.html`, with a GitHub Actions Pages deployment workflow at `.github/workflows/pages-demo.yml`. It demonstrates navigation, project/document cards, a chat empty state, source evidence cards, and illustrative PDF/table deep-link interactions.

This preview is intentionally disconnected from FastAPI, PostgreSQL, JWT authentication, Bionic server routes and Agent Runtime. All displayed records and sample assistant responses are fictional. It is a design review surface, not a deployed or functional Personal Expert AI instance.

To enable deployment, set repository Settings → Pages → Build and deployment → Source to **GitHub Actions**. The preview URL will be `https://dach1980.github.io/bionic-personal-expert-ai/` once the Pages workflow succeeds.
