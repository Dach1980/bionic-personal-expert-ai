# Bionic project checkpoint — 2026-11-15

## Milestone: Keycloak login and Bionic UI access

**Status: achieved (UI access); chat completion not yet tested.**

On 2026-11-15 the user successfully signed in to the local Bionic UI through Keycloak/OAuth2 Proxy.

Verified during the session:
- Keycloak client `bionic-gpt-client` in realm `bionic-gpt` is enabled and uses the standard OIDC flow.
- The direct audience mapper `pea-api-audience` is configured to add audience `pea-api` to access tokens.
- The mapper was found in the client's effective protocol-mapper evaluation.
- After signing out and signing in again, the user could open Bionic at `http://localhost:30000/`.
- The Bionic logs queried for the last five minutes contained no `audience mismatch` or `Keycloak JWT authentication rejected` messages.
- Diagnostic code was committed as `05d128f844d2578454270e59a6c174e66d68d5ef` (Log safe Keycloak JWT audience diagnostics).

This confirms successful login and UI access. It does **not** yet prove that chat completion requests work: a model must be configured and a real message/response tested.

## LM Studio: source-code finding

The current Bionic source already supports a custom OpenAI-compatible model. A code extension is not the first step.

Relevant source:
- `crates/web-pages/models/select_provider.rs`: the Select Provider page includes a **Custom Model** link.
- `crates/web-pages/models/upsert.rs`: the custom model form supports:
  - Model Name / Display Name
  - Model Type
  - API Adapter, including `OpenAICompatible`
  - Base URL
  - API Key
  - Context size and model capabilities
- `crates/web-server/handlers/models.rs`: model create/update handlers.
- `crates/web-pages/providers/upsert.rs` and `crates/web-server/handlers/providers.rs`: system-admin provider management, including custom provider records.

The fixed **Create Model: OpenAI** dialog is a preset. It intentionally hides the Base URL and Model fields, so it is not the correct form for LM Studio.

**Next configuration route:** on the Select Provider page, use **Custom Model** and select **OpenAI Compatible**. Do not save until the actual LM Studio model identifier and a network-reachable Base URL have been confirmed.

LM Studio's host-side `127.0.0.1:1234` is not automatically reachable as `127.0.0.1` from a Bionic pod. The Base URL must be reachable from the Kubernetes workload. Verify connectivity before saving. API-key requirements depend on the LM Studio server configuration; do not enter a real cloud API key.

Bionic's first-model setup may also configure an embeddings model from the provider's embedding default or use the local Ollama fallback. This dependency must be checked during model setup; do not assume that configuring the chat model alone completes all model setup.

## Local environment and shutdown safety

Known local environment:
- k3d cluster: `k3d-bionic`
- Bionic namespace: `bionic-gpt`
- Keycloak namespace: `keycloak`
- Bionic UI NodePort: `http://localhost:30000/`
- Local repository checkout previously used: `/mnt/d/Projects/bionic-personal-expert-ai`

The Git repository stores source code, deployment manifests, and helper scripts. It does **not** by itself contain a complete backup of the running local environment. Runtime state such as PostgreSQL/Keycloak database contents, Kubernetes Secrets, persistent volumes, kubeconfig, and local environment files lives in the local Docker/k3d environment and/or local files. Do not assume that cloning the repository alone recreates the currently working login state.

A normal computer shutdown should not be confused with deleting the cluster or its Docker volumes. To preserve the current local installation:
- Do not run `.agents/skills/k3d-environment/dev-init.sh`: it explicitly deletes and recreates `k3d-bionic`.
- Do not delete the k3d cluster, Docker volumes, or persistent-volume data.
- Keep local kubeconfig and secret material private; never commit tokens, passwords, cookies, private keys, or populated `.env` files.
- After restarting the computer and Docker Desktop, verify that the existing cluster and namespaces are still present before attempting redeployment.
- The repository's `.agents/skills/k3d-environment/dev-setup.sh` is an initialization/deployment workflow, not a verified safe restart command. Do not run it blindly against the existing cluster.

## Next actions

1. On the Select Provider page, open **Custom Model** (not the OpenAI preset).
2. Confirm the model identifier currently served by LM Studio.
3. Determine and test the Base URL reachable from the Bionic pod.
4. Configure the OpenAI-compatible chat model.
5. Resolve the embeddings model requirement/fallback.
6. Send a simple chat message and confirm a backend response.
7. Once the current local state is known to survive a normal restart, document a tested start/stop procedure and a separate backup/restore procedure.

## Source snapshot

- Repository: https://github.com/Dach1980/bionic-personal-expert-ai
- Branch: `main`
- Source tree HEAD inspected for this checkpoint: `05d128f844d2578454270e59a6c174e66d68d5ef`
