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
