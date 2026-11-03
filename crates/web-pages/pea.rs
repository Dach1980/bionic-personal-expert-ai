#![allow(non_snake_case)]

use crate::app_layout::{Layout, SideBar};
use db::authz::Rbac;
use dioxus::prelude::*;

pub fn page(rbac: Rbac, team_id: String, tenant_id: String, tab: &str, locale: &str) -> String {
    let selected_item = match tab {
        "reviews" => SideBar::Reviews,
        "knowledge" => SideBar::KnowledgeBase,
        _ => SideBar::Documents,
    };
    let page = rsx! {
        Layout {
            section_class: "p-4",
            selected_item,
            team_id: team_id.clone(),
            rbac,
            title: "Project Workspace".to_string(),
            locale: Some(locale.to_string()),
            header: rsx!(
                div {
                    class: "flex flex-col",
                    h1 { class: "text-lg font-semibold", "Project Workspace" }
                    p { class: "text-xs text-base-content/60", "Personal Expert AI · project documentation and evidence" }
                }
            ),
            div {
                id: "pea-workspace",
                "data-tenant-id": tenant_id,
                "data-tab": tab,
                class: "mx-auto w-full max-w-7xl space-y-4",
                div {
                    class: "rounded-xl border border-base-300 bg-base-100 p-4",
                    h2 { class: "text-base font-semibold mb-1", "Projects" }
                    p { class: "text-sm text-base-content/60 mb-3", "Choose a project to browse its documents, versions, evidence and knowledge." }
                    div { id: "pea-projects", class: "flex flex-wrap gap-2" }
                }
                div {
                    class: "grid grid-cols-1 xl:grid-cols-2 gap-4",
                    div {
                        class: "rounded-xl border border-base-300 bg-base-100 p-4 min-w-0",
                        div { class: "flex items-center justify-between mb-3",
                            h2 { class: "text-base font-semibold", "Project contents" }
                            span { id: "pea-breadcrumb", class: "text-xs text-base-content/50" }
                        }
                        div { id: "pea-content", class: "space-y-2 text-sm" }
                    }
                    div {
                        class: "rounded-xl border border-base-300 bg-base-100 p-4 min-w-0",
                        h2 { class: "text-base font-semibold mb-3", "Source preview" }
                        div { id: "pea-preview", class: "space-y-3 text-sm" }
                    }
                }
                div {
                    id: "pea-status",
                    role: "status",
                    "aria-live": "polite",
                    class: "text-xs text-base-content/60"
                }
            }
        }
    };
    crate::render(page)
}
