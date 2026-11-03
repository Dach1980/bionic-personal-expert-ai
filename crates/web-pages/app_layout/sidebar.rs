use super::{SideBar, SidebarLabels, SidebarParams};
use crate::menu::{NavGroup, NavItem};
use assets::files::*;
use dioxus::prelude::*;

pub fn render(params: &SidebarParams, labels: &SidebarLabels) -> Element {
    let selected_item = params.selected_item.to_string();
    let history_label = labels.history.clone();
    let team_id = params.team_id.clone();
    let rbac = &params.rbac;
    let can_view_chats = params.can_view_chats;
    let can_view_chat_history = params.can_view_chat_history;
    let setup_required = params.setup_required;

    rsx!(
        // Projects remain backed by Bionic's existing project routes and RBAC.
        if rbac.can_manage_projects() {
            super::project_sidebar::ProjectSidebar {
                team_id: team_id.clone(),
                projects: rbac.projects.clone(),
                selected_project_id: params.selected_project_id,
                index_selected: params.selected_item == SideBar::Projects
                    && params.selected_project_id.is_none(),
                disabled: setup_required,
            }
        }

        if can_view_chats || can_view_chat_history || rbac.can_manage_projects() {
            NavGroup {
                heading: "Workspace",
                content: rsx!(
                    NavItem {
                        id: SideBar::Documents.to_string(),
                        selected_item_id: selected_item.clone(),
                        href: format!("{}?tab=documents", crate::routes::pea::Index { team_id: team_id.clone() }),
                        icon: nav_ccsds_data_svg.name,
                        title: "Documents",
                        disabled: setup_required
                    }
                    NavItem {
                        id: SideBar::Reviews.to_string(),
                        selected_item_id: selected_item.clone(),
                        href: format!("{}?tab=reviews", crate::routes::pea::Index { team_id: team_id.clone() }),
                        icon: nav_audit_svg.name,
                        title: "Reviews",
                        disabled: setup_required
                    }
                    NavItem {
                        id: SideBar::KnowledgeBase.to_string(),
                        selected_item_id: selected_item.clone(),
                        href: format!("{}?tab=knowledge", crate::routes::pea::Index { team_id: team_id.clone() }),
                        icon: nav_ccsds_data_svg.name,
                        title: "Knowledge Base",
                        disabled: setup_required
                    }
                    if can_view_chats {
                        NavItem {
                            id: SideBar::Skills.to_string(),
                            selected_item_id: selected_item.clone(),
                            href: crate::routes::skills::Index { team_id: team_id.clone() },
                            icon: nav_automations_svg.name,
                            title: "Skills",
                            disabled: setup_required
                        }
                    }
                )
            }
        }

        if can_view_chats || can_view_chat_history {
            NavGroup {
                heading: "Dialogues",
                content: rsx!(
                    if can_view_chats {
                        NavItem {
                            id: SideBar::Console.to_string(),
                            selected_item_id: selected_item.clone(),
                            href: crate::routes::console::Index { team_id: team_id.clone() },
                            icon: nav_service_requests_svg.name,
                            title: "New dialogue",
                            disabled: setup_required
                        }
                    }
                    if can_view_chat_history {
                        NavItem {
                            id: SideBar::History.to_string(),
                            selected_item_id: selected_item.clone(),
                            href: crate::routes::history::Index { team_id: team_id.clone() },
                            icon: nav_history_svg.name,
                            title: history_label,
                            disabled: setup_required
                        }
                    }
                )
            }
        }
    )
}
