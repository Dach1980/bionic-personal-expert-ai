type JsonRecord = Record<string, any>;

export function peaWorkspace(): void {
    const root = document.querySelector<HTMLElement>("#pea-workspace");
    if (!root) return;

    const tenantId = root.dataset.tenantId || "";
    const activeTab = new URLSearchParams(window.location.search).get("tab")
        || root.dataset.tab
        || "documents";
    const projectsHost = document.querySelector<HTMLElement>("#pea-projects");
    const contentHost = document.querySelector<HTMLElement>("#pea-content");
    const previewHost = document.querySelector<HTMLElement>("#pea-preview");
    const statusHost = document.querySelector<HTMLElement>("#pea-status");
    const breadcrumb = document.querySelector<HTMLElement>("#pea-breadcrumb");

    let selectedProject: JsonRecord | null = null;

    const setStatus = (message: string): void => {
        if (statusHost) statusHost.textContent = message;
    };

    const element = (tag: string, className = "", text = ""): HTMLElement => {
        const node = document.createElement(tag);
        if (className) node.className = className;
        if (text) node.textContent = text;
        return node;
    };

    const button = (text: string, onClick: () => void, selected = false): HTMLButtonElement => {
        const node = document.createElement("button");
        node.type = "button";
        node.className = selected
            ? "btn btn-sm btn-primary"
            : "btn btn-sm btn-outline";
        node.textContent = text;
        node.addEventListener("click", onClick);
        return node;
    };

    const api = async (path: string): Promise<any> => {
        const response = await fetch(path, {
            method: "GET",
            credentials: "same-origin",
            headers: { "Accept": "application/json" },
        });
        if (!response.ok) {
            const message = await response.text();
            throw new Error(`API ${response.status}: ${message.slice(0, 300)}`);
        }
        return response.json();
    };

    const base = (): string => `/api/pea/tenants/${encodeURIComponent(tenantId)}`;
    const projectBase = (): string => {
        if (!selectedProject) throw new Error("Select a project first");
        return `${base()}/projects/${encodeURIComponent(selectedProject.id)}`;
    };

    const showEmpty = (host: HTMLElement | null, message: string): void => {
        if (!host) return;
        host.replaceChildren(element("p", "text-sm text-base-content/60 py-3", message));
    };

    const renderProjects = async (): Promise<void> => {
        if (!projectsHost) return;
        setStatus("Loading projects…");
        projectsHost.replaceChildren();
        const projects = await api(`${base()}/projects`) as JsonRecord[];
        if (!projects.length) {
            projectsHost.append(element("p", "text-sm text-base-content/60", "No projects are available in this tenant."));
        }
        projects.forEach((project) => {
            projectsHost.append(button(
                project.name || project.id,
                () => selectProject(project),
                selectedProject?.id === project.id,
            ));
        });
        setStatus(`Loaded ${projects.length} project(s).`);
    };

    const selectProject = async (project: JsonRecord): Promise<void> => {
        selectedProject = project;
        if (breadcrumb) breadcrumb.textContent = project.name || project.id;
        if (previewHost) showEmpty(previewHost, "Choose a document version to inspect its source pages.");
        await renderProjects();
        if (activeTab === "reviews") {
            await renderReviews();
        } else if (activeTab === "knowledge") {
            await renderKnowledge();
        } else {
            await renderDocuments();
        }
    };

    const renderDocuments = async (): Promise<void> => {
        if (!contentHost) return;
        contentHost.replaceChildren();
        setStatus("Loading project documents…");
        const documents = await api(`${projectBase()}/documents`) as JsonRecord[];
        if (!documents.length) {
            showEmpty(contentHost, "This project has no documents yet.");
            setStatus("No documents found.");
            return;
        }
        documents.forEach((document) => {
            const row = element("div", "rounded-lg border border-base-300 p-3");
            row.append(element("div", "font-medium", document.title || document.id));
            row.append(element("div", "text-xs text-base-content/60 mt-1", `Status: ${document.status || "unknown"}`));
            row.append(button("View versions", () => renderVersions(document)));
            contentHost.append(row);
        });
        setStatus(`Loaded ${documents.length} document(s).`);
    };

    const renderVersions = async (doc: JsonRecord): Promise<void> => {
        if (!contentHost) return;
        contentHost.replaceChildren();
        if (breadcrumb) breadcrumb.textContent = `${selectedProject?.name || "Project"} / ${doc.title || "Document"}`;
        setStatus("Loading immutable document versions…");
        const versions = await api(`${projectBase()}/documents/${encodeURIComponent(doc.id)}/versions`) as JsonRecord[];
        if (!versions.length) {
            showEmpty(contentHost, "No uploaded versions are available.");
            return;
        }
        versions.forEach((version) => {
            const row = element("div", "rounded-lg border border-base-300 p-3");
            row.append(element("div", "font-medium", version.original_filename || version.id));
            row.append(element("div", "text-xs text-base-content/60 mt-1", `Version status: ${version.status || "unknown"} · ${version.uploaded_at || ""}`));
            row.append(button("Inspect version", () => renderVersion(doc, version)));
            contentHost.append(row);
        });
        setStatus(`Loaded ${versions.length} version(s).`);
    };

    const renderVersion = async (doc: JsonRecord, version: JsonRecord): Promise<void> => {
        if (!contentHost || !previewHost) return;
        contentHost.replaceChildren();
        previewHost.replaceChildren();
        if (breadcrumb) breadcrumb.textContent = `${selectedProject?.name || "Project"} / ${doc.title || "Document"} / ${version.original_filename || "Version"}`;
        setStatus("Loading source fragments and rendered pages…");
        const docPath = `/documents/${encodeURIComponent(doc.id)}/versions/${encodeURIComponent(version.id)}`;
        const [fragments, artifacts] = await Promise.all([
            api(`${projectBase()}${docPath}/source-fragments`) as Promise<JsonRecord[]>,
            api(`${projectBase()}${docPath}/artifacts`) as Promise<JsonRecord[]>,
        ]);
        const pageByNumber = new Map<string, HTMLElement>();
        artifacts.forEach((artifact) => {
            const locator = artifact.locator || {};
            const pageNumber = String(locator.page ?? locator.page_number ?? "");
            const figure = element("figure", "rounded-lg border border-base-300 p-2");
            figure.dataset.page = pageNumber;
            const image = document.createElement("img");
            image.loading = "lazy";
            image.alt = pageNumber ? `Document page ${pageNumber}` : "Rendered document page";
            image.className = "w-full h-auto rounded";
            image.src = `${projectBase()}${docPath}/artifacts/${encodeURIComponent(artifact.id)}/content`;
            figure.append(image);
            figure.append(element("figcaption", "text-xs text-base-content/60 mt-2", pageNumber ? `Page ${pageNumber}` : "Rendered page"));
            previewHost.append(figure);
            if (pageNumber) pageByNumber.set(pageNumber, figure);
        });

        if (!fragments.length) {
            contentHost.append(element("p", "text-sm text-base-content/60", "No source fragments have been extracted for this version yet."));
        }
        fragments.forEach((fragment) => {
            const row = element("button", "block w-full text-left rounded-lg border border-base-300 p-3 hover:border-primary");
            row.type = "button";
            const locator = fragment.locator || {};
            const pageNumber = String(locator.page ?? locator.page_number ?? "");
            const position = pageNumber ? `Page ${pageNumber}` : (locator.cell || locator.sheet || fragment.fragment_type || "Source");
            row.append(element("div", "text-xs font-semibold text-primary mb-1", `${fragment.fragment_type || "fragment"} · ${position}`));
            row.append(element("div", "text-sm whitespace-pre-wrap", fragment.excerpt || "No excerpt available."));
            row.addEventListener("click", () => {
                contentHost.querySelectorAll("button").forEach((item) => item.classList.remove("border-primary", "bg-primary/5"));
                row.classList.add("border-primary", "bg-primary/5");
                const page = pageByNumber.get(pageNumber);
                if (page) page.scrollIntoView({ behavior: "smooth", block: "center" });
                setStatus(`Selected source fragment ${fragment.id}`);
            });
            contentHost.append(row);
        });
        setStatus(`Loaded ${fragments.length} source fragment(s) and ${artifacts.length} rendered page(s).`);
    };

    const renderReviews = async (): Promise<void> => {
        if (!contentHost) return;
        contentHost.replaceChildren();
        setStatus("Loading reviews…");
        const reviews = await api(`${projectBase()}/reviews`) as JsonRecord[];
        if (!reviews.length) {
            showEmpty(contentHost, "No reviews have been created for this project yet.");
            return;
        }
        reviews.forEach((review) => {
            const row = element("div", "rounded-lg border border-base-300 p-3");
            row.append(element("div", "font-medium", review.title || review.id));
            row.append(element("div", "text-xs text-base-content/60 mt-1", `${review.status} · ${review.scope}`));
            row.append(button("Show findings", async () => {
                const findings = await api(`${projectBase()}/reviews/${encodeURIComponent(review.id)}/findings`) as JsonRecord[];
                contentHost.replaceChildren(row);
                if (!findings.length) row.append(element("p", "text-xs text-base-content/60 mt-2", "No findings."));
                findings.forEach((finding) => {
                    const item = element("div", "mt-3 rounded border border-base-300 p-2");
                    item.append(element("div", "font-medium", `${finding.severity} · ${finding.title}`));
                    item.append(element("p", "text-xs text-base-content/70 mt-1", finding.description || ""));
                    item.append(button("Show evidence", async () => {
                        const evidence = await api(`${projectBase()}/findings/${encodeURIComponent(finding.id)}/evidence`) as JsonRecord[];
                        if (previewHost) {
                            previewHost.replaceChildren(element("h3", "font-semibold", "Evidence"));
                            evidence.forEach((entry) => {
                                const card = element("div", "rounded border border-base-300 p-3");
                                card.append(element("div", "text-xs font-semibold", `${entry.relation} · ${entry.evidence_type}`));
                                card.append(element("p", "text-sm whitespace-pre-wrap mt-1", entry.excerpt || entry.reference_label || "Linked source evidence"));
                                previewHost.append(card);
                            });
                        }
                    }));
                    row.append(item);
                });
            }));
            contentHost.append(row);
        });
        setStatus(`Loaded ${reviews.length} review(s).`);
    };

    const renderKnowledge = async (): Promise<void> => {
        if (!contentHost) return;
        contentHost.replaceChildren();
        setStatus("Loading project knowledge…");
        const items = await api(`${projectBase()}/knowledge`) as JsonRecord[];
        if (!items.length) {
            showEmpty(contentHost, "No knowledge records are available for this project yet.");
            return;
        }
        items.forEach((item) => {
            const card = element("div", "rounded-lg border border-base-300 p-3");
            card.append(element("div", "font-medium", item.title || item.name || item.id || "Knowledge item"));
            const summary = item.summary || item.content || item.description || "";
            card.append(element("p", "text-sm text-base-content/70 mt-1 whitespace-pre-wrap", String(summary).slice(0, 2000)));
            contentHost.append(card);
        });
        setStatus(`Loaded ${items.length} knowledge item(s).`);
    };

    if (!tenantId) {
        setStatus("The Keycloak access token has no tenant_id claim. Ask an administrator to configure the PEA tenant attribute.");
        return;
    }

    renderProjects()
        .then(() => {
            if (selectedProject) return;
            if (contentHost) {
                const message = activeTab === "reviews"
                    ? "Select a project to browse its reviews."
                    : activeTab === "knowledge"
                        ? "Select a project to browse its knowledge base."
                        : "Select a project to browse its documents.";
                showEmpty(contentHost, message);
            }
        })
        .catch((error: unknown) => setStatus(error instanceof Error ? error.message : "Unable to load projects."));
}
