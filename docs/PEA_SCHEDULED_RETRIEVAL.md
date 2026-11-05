# Personal Expert AI retrieval in scheduled Bionic tasks

The scheduled worker can expose `pea_search` only when PEA has authorized the task.

## Runtime flow

1. The cron worker loads the scheduled task from Bionic's database and obtains the owner's stored Keycloak `openid_sub`.
2. If the task has a Bionic project ID and all PEA environment settings are present, it obtains a short-lived access token using the confidential `pea-bionic-worker` Keycloak client and OAuth2 client credentials.
3. It calls PEA's internal delegation endpoint with the Bionic project ID, scheduled task ID and owner `sub`.
4. PEA resolves the explicit project binding, maps the owner by exact `(issuer, sub)`, and independently checks active tenant/project access.
5. PEA returns a short-lived search grant. The raw grant remains only in the current worker process memory.
6. The scheduled Tool Execution Loop receives an immutable `PeaRetrievalContext`. Only then does `pea_search` appear in the model's tool definitions and dispatcher.
7. Each search sends a fresh workload access token and the task-bound grant to PEA. PEA rechecks the grant, owner, binding and current permissions before returning source fragments.

The grant is issued per run with a 15-minute lifetime. No user's refresh token or PEA grant token is persisted in Bionic's database.

## Model-visible contract

The model can provide only:

- `query`: 2–1000 characters;
- `top_k`: 1–20, default 8.

The model cannot provide or override tenant ID, PEA project UUID, Bionic project ID, scheduled task ID, owner subject, Bionic instance ID, grant token, API URL or Keycloak credentials.

Search results retain source fragment IDs, document title, original filename, locator and content hash. Excerpts are labelled as untrusted source material. The system prompt instructs the model not to follow commands embedded in retrieved documents and to cite only exact returned fragment IDs.

## Failure behavior

- No PEA environment configuration: scheduled task continues without `pea_search`.
- Missing/disabled project binding: PEA declines grant issuance; scheduled task continues without `pea_search`.
- Unknown/inactive owner or missing PEA membership/project access: PEA declines grant issuance; scheduled task continues without `pea_search`.
- Invalid, expired, revoked or mismatched grant: PEA rejects search.
- Oversized response, timeout, redirect or non-HTTPS production endpoint: the tool fails closed.
- No fallback to cloud retrieval or another project.

The absence of `pea_search` is deliberate and must not be interpreted by the agent as proof that the PEA project contains no relevant documents.

## Environment

Configure only the Bionic **cron** service:

- `PEA_API_BASE_URL`: private PEA API origin;
- `PEA_OIDC_TOKEN_URL`: Keycloak realm token endpoint;
- `PEA_WORKLOAD_CLIENT_ID=pea-bionic-worker`;
- `PEA_WORKLOAD_CLIENT_SECRET`: secret-manager value created/rotated in Keycloak.

The setup template is `infra-as-code/stack.yaml`; local development values are in `infra-as-code/.env.example`. Never put the client secret in Git, browser configuration, model prompts, logs or scheduled task text.

For a local Stack dev namespace, after generating the secret in Keycloak, create/update the Kubernetes secret without putting its value in a command history or Git file. For example, load it from a protected shell variable and apply it to namespace `bionic-gpt`:

```powershell
kubectl -n bionic-gpt create secret generic pea-bionic-worker --from-literal=client_secret="$env:PEA_WORKLOAD_CLIENT_SECRET" --dry-run=client -o yaml | kubectl apply -f -
```

For UAT/production, use the deployment's managed secret store and the same secret name/key expected by the Stack manifest.

PEA must independently be configured with:

- `PEA_WORKLOAD_CLIENT_ID=pea-bionic-worker`;
- `PEA_BIONIC_INSTANCE_ID` matching the explicit binding namespace;
- its regular Keycloak issuer, audience and JWKS settings.

## Compatibility

The ordinary interactive SSE chat is deliberately unchanged:
- it calls `create_request(..., None)`;
- its tool definitions remain the fixed existing catalog;
- `pea_search` is not registered for interactive SSE;
- the new retrieval context is passed only through `run_scheduled_chat` and the scheduled Tool Execution Loop.

## Deployment checklist

1. Run PEA's `scripts/configure_keycloak_realm.py` against the existing Stack-managed `bionic-gpt` realm.
2. Generate/rotate the `pea-bionic-worker` client secret and store it in the cron deployment secret manager.
3. Configure PEA's workload client allowlist and Bionic instance ID.
4. Apply PEA migration 0015.
5. Create each Bionic→PEA project binding through PEA's system-admin API.
6. Verify one scheduled task with an authorized owner and one denied task with an unprovisioned or unauthorized owner.
7. Verify that a task without a project binding receives no `pea_search` tool.
8. Verify the old SSE chat still works and has no PEA retrieval tool.

Repository CI cannot verify live Keycloak configuration, deployment secrets, network reachability or the private PEA database. Those checks remain an explicit deployment gate.
