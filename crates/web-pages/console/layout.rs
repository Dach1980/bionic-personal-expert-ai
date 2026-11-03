#![allow(non_snake_case)]
use crate::app_layout::{Layout, SideBar};
use db::authz::Rbac;
use db::queries::capabilities::Capability;
use db::queries::models::ModelConfig;
use dioxus::prelude::*;

use super::console_stream::latest_mcp_app;
use super::{ChatWithChunks, PendingChatState};

#[component]
pub fn ConsoleLayout(
    team_id: String,
    conversation_id: Option<i64>,
    rbac: Rbac,
    chat_history: Vec<ChatWithChunks>,
    pending_chat_state: PendingChatState,
    prompt: ModelConfig,
    selected_item: SideBar,
    title: String,
    header: Element,
    is_tts_disabled: bool,
    capabilities: Vec<Capability>,
    #[props(default)] selected_project_id: Option<i32>,
) -> Element {
    let has_pending_chat = pending_chat_state.shall_we_call_the_model();
    let latest_app = conversation_id.and_then(|_| latest_mcp_app(&chat_history));

    rsx! {
        Layout {
            section_class: "console flex flex-col justify-start h-[calc(100%-79px)]",
            selected_item,
            team_id: team_id.clone(),
            rbac: rbac.clone(),
            title,
            header,
            selected_project_id,
            div {
                id: "console-panel",
                class: "h-full min-h-0 flex flex-col lg:flex-row",
                div {
                    class: "flex min-h-0 min-w-0 flex-1 flex-col",
                    if !chat_history.is_empty() || has_pending_chat {
                        super::console_stream::ConsoleStream {
                            team_id: team_id.clone(),
                            chat_history,
                            pending_chat_state: pending_chat_state.clone(),
                            is_tts_disabled,
                            rbac: rbac.clone()
                        }
                        div {
                            super::prompt_form::Form {
                                team_id: team_id.clone(),
                                model_id: prompt.id,
                                lock_console: has_pending_chat,
                                conversation_id,
                                disclaimer: prompt.disclaimer,
                                capabilities: capabilities.clone(),
                            }
                        }
                    } else {
                        div {
                            class: "flex-1 flex flex-col justify-center h-full",
                            div {
                                class: "mx-auto mb-8 w-full max-w-3xl px-4",
                                div {
                                    class: "mb-3 flex items-center gap-2 text-sm font-medium text-primary",
                                    "PERSONAL EXPERT AI"
                                }
                                h1 {
                                    class: "mb-3 text-center text-3xl font-semibold tracking-tight",
                                    "Your project knowledge, ready to explore"
                                }
                                p {
                                    class: "mx-auto mb-6 max-w-2xl text-center text-sm text-base-content/60",
                                    "Ask questions about your project documentation, trace answers to their sources, and continue your work with AI."
                                }
                                div {
                                    class: "mb-6 grid grid-cols-1 gap-3 sm:grid-cols-3",
                                    div {
                                        class: "rounded-xl border border-base-300 bg-base-100 p-3 text-sm",
                                        div { class: "mb-1 font-medium", "Explore projects" }
                                        p { class: "text-xs text-base-content/60", "Keep project conversations and context together." }
                                    }
                                    div {
                                        class: "rounded-xl border border-base-300 bg-base-100 p-3 text-sm",
                                        div { class: "mb-1 font-medium", "Check documentation" }
                                        p { class: "text-xs text-base-content/60", "Prepare evidence-based reviews from source files." }
                                    }
                                    div {
                                        class: "rounded-xl border border-base-300 bg-base-100 p-3 text-sm",
                                        div { class: "mb-1 font-medium", "Trace every answer" }
                                        p { class: "text-xs text-base-content/60", "Open the source page or fragment behind a finding." }
                                    }
                                }
                            }
                            div {
                                super::prompt_form::Form {
                                    team_id: team_id.clone(),
                                    model_id: prompt.id,
                                    lock_console: has_pending_chat,
                                    conversation_id,
                                    disclaimer: prompt.disclaimer,
                                    capabilities,
                                }
                            }
                        }
                    }
                }
                if let Some(app) = latest_app {
                    super::console_stream::PersistentMcpAppPanel {
                        key: "{team_id}-{conversation_id:?}",
                        team_id: team_id.clone(),
                        conversation_id: conversation_id.unwrap_or_default(),
                        app
                    }
                }
            }
        }
    }
}
