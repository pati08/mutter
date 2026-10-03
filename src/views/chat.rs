//! The chat screen: contact list, message thread, and composer.
//!
//! The thread lives only in memory for the session; a reload clears it. Incoming
//! messages are deduped by id and acknowledged in batches (a replay burst is acked
//! together). See `PLAN.md` M3.

use std::collections::HashMap;

use dioxus::prelude::*;
use futures::FutureExt;

use crate::api::auth::logout;
use crate::api::chat::{ack_messages, contacts, events, send_message};
use crate::models::{Message, User, UserId};
use crate::routes::Route;
use crate::views::RequireAuth;

const CHAT_ICON: Asset = asset!("/assets/chat.svg");

#[component]
pub fn Home() -> Element {
    rsx! { RequireAuth { ChatScreen {} } }
}

#[component]
fn ChatScreen() -> Element {
    let mut thread = use_signal(Vec::<Message>::new);
    let mut selected = use_signal(|| None::<UserId>);
    let mut unread = use_signal(HashMap::<UserId, u32>::new);
    let mut draft = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let contact_list = use_resource(move || async move { contacts().await });

    // Consume the SSE stream once, on the client only. `use_effect` does not run
    // during SSR and, with no reactive reads, will not restart.
    use_effect(move || {
        spawn(async move {
            let mut stream = match events().await {
                Ok(stream) => stream,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    return;
                }
            };

            while let Some(item) = stream.recv().await {
                let Ok(message) = item else { continue };

                let mut ids = vec![message.id];
                absorb(&mut thread, &mut unread, *selected.peek(), message);

                // Ack anything already buffered (e.g. a replay burst) together.
                while let Some(Some(Ok(next))) = stream.recv().now_or_never() {
                    ids.push(next.id);
                    absorb(&mut thread, &mut unread, *selected.peek(), next);
                }

                let _ = ack_messages(ids).await;
            }
        });
    });

    let on_send = move |event: FormEvent| async move {
        event.prevent_default();

        let Some(to) = *selected.peek() else { return };
        let text = draft();
        if text.trim().is_empty() {
            return;
        }

        draft.set(String::new());
        match send_message(to, text).await {
            Ok(message) => push_unique(&mut thread, message),
            Err(err) => error.set(Some(err.to_string())),
        }
    };

    let selected_id = *selected.peek();
    let thread_snapshot = thread();
    let selected_user = match contact_list() {
        Some(Ok(list)) => selected_id.and_then(|id| list.into_iter().find(|user| user.id == id)),
        _ => None,
    };
    let conversation: Vec<Message> = match selected_id {
        Some(id) => thread_snapshot
            .iter()
            .filter(|message| message.from == id || message.to == id)
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let conversation_empty = conversation.is_empty();

    rsx! {
        div { class: "flex h-dvh bg-slate-100 text-slate-900",
            aside { class: "flex w-80 shrink-0 flex-col border-r border-slate-200 bg-white",
                header { class: "flex items-center justify-between px-5 py-4",
                    span { class: "text-xl font-semibold", "mutter" }
                    SignOutButton {}
                }

                div { class: "px-4 pb-3",
                    input {
                        class: "w-full rounded-full border border-slate-200 bg-slate-100 px-4 py-2 text-sm text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-400",
                        placeholder: "Search here...",
                    }
                }

                nav { class: "flex-1 space-y-1 overflow-y-auto px-2 pb-3",
                    match contact_list() {
                        Some(Ok(list)) => rsx! {
                            for user in list {
                                ContactRow {
                                    key: "{user.id.0}",
                                    user: user.clone(),
                                    last: last_message(&thread_snapshot, user.id),
                                    unread: unread().get(&user.id).copied().unwrap_or(0),
                                    selected: selected_id == Some(user.id),
                                    on_select: move |id: UserId| {
                                        selected.set(Some(id));
                                        unread.write().remove(&id);
                                    },
                                }
                            }
                        },
                        Some(Err(err)) => rsx! {
                            p { class: "px-3 py-3 text-sm text-red-600", "{err}" }
                        },
                        None => rsx! {
                            p { class: "px-3 py-3 text-sm text-slate-400", "Loading contacts…" }
                        },
                    }
                }
            }

            main { class: "flex flex-1 flex-col bg-slate-50",
                if let Some(message) = error() {
                    div { class: "bg-red-50 px-6 py-2 text-sm text-red-700", "{message}" }
                }

                match selected_user {
                    Some(user) => rsx! {
                        header { class: "flex items-center gap-3 border-b border-slate-200 bg-white px-6 py-4",
                            div { class: "flex h-10 w-10 items-center justify-center rounded-full bg-indigo-500 text-sm font-semibold text-white",
                                "{initials(&user.name)}"
                            }
                            div {
                                div { class: "font-semibold", "{user.name}" }
                                div { class: "text-sm text-slate-500", "@{user.username}" }
                            }
                        }

                        div { class: "flex-1 space-y-3 overflow-y-auto px-6 py-6",
                            for message in conversation {
                                Bubble {
                                    key: "{message.id.0}",
                                    mine: message.from != user.id,
                                    contents: message.contents,
                                    sent: message.sent.0,
                                }
                            }
                            if conversation_empty {
                                p { class: "text-center text-sm text-slate-400", "No messages yet. Say hello." }
                            }
                        }

                        form {
                            class: "flex items-center gap-3 border-t border-slate-200 bg-white px-6 py-4",
                            onsubmit: on_send,
                            input {
                                class: "flex-1 rounded-full border border-slate-200 bg-slate-100 px-4 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-400",
                                placeholder: "Type your message",
                                value: "{draft}",
                                oninput: move |event| draft.set(event.value()),
                            }
                            button {
                                r#type: "submit",
                                class: "rounded-full bg-indigo-600 px-5 py-2 font-medium text-white hover:bg-indigo-700",
                                "Send"
                            }
                        }
                    },
                    None => rsx! { EmptyState {} },
                }
            }
        }
    }
}

#[component]
fn ContactRow(
    user: User,
    last: Option<String>,
    unread: u32,
    selected: bool,
    on_select: EventHandler<UserId>,
) -> Element {
    let id = user.id;
    let preview = last.unwrap_or_else(|| format!("@{}", user.username));

    rsx! {
        button {
            class: if selected {
                "flex w-full items-center gap-3 rounded-xl bg-indigo-50 px-3 py-2 text-left"
            } else {
                "flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left hover:bg-slate-50"
            },
            onclick: move |_| on_select.call(id),
            div { class: "flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-indigo-500 text-sm font-semibold text-white",
                "{initials(&user.name)}"
            }
            div { class: "min-w-0 flex-1",
                div { class: "truncate font-medium", "{user.name}" }
                div { class: "truncate text-sm text-slate-500", "{preview}" }
            }
            if unread > 0 {
                span { class: "ml-2 rounded-full bg-indigo-600 px-2 py-0.5 text-xs font-semibold text-white",
                    "{unread}"
                }
            }
        }
    }
}

#[component]
fn Bubble(mine: bool, contents: String, sent: u64) -> Element {
    rsx! {
        div { class: if mine { "flex justify-end" } else { "flex justify-start" },
            div {
                class: if mine {
                    "max-w-md rounded-2xl rounded-br-sm bg-indigo-600 px-4 py-2 text-white"
                } else {
                    "max-w-md rounded-2xl rounded-bl-sm bg-white px-4 py-2 text-slate-800 shadow-sm"
                },
                p { class: "whitespace-pre-wrap break-words", "{contents}" }
                div {
                    class: if mine {
                        "mt-1 text-right text-[11px] text-indigo-200"
                    } else {
                        "mt-1 text-right text-[11px] text-slate-400"
                    },
                    "{relative_time(sent)}"
                }
            }
        }
    }
}

#[component]
fn EmptyState() -> Element {
    rsx! {
        div { class: "flex flex-1 items-center justify-center",
            div { class: "text-center",
                div { class: "mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-full bg-indigo-100",
                    img { src: CHAT_ICON, class: "h-7 w-7", alt: "" }
                }
                h2 { class: "text-lg font-semibold text-slate-700", "Select a conversation" }
                p { class: "mt-1 text-sm text-slate-500", "Messages are session-only and never stored." }
            }
        }
    }
}

#[component]
fn SignOutButton() -> Element {
    let navigator = use_navigator();

    rsx! {
        button {
            class: "rounded-lg border border-slate-300 px-2.5 py-1 text-xs font-medium text-slate-600 hover:bg-slate-100",
            onclick: move |_| async move {
                let _ = logout().await;
                let _ = navigator.replace(Route::Login {});
            },
            "Sign out"
        }
    }
}

/// Appends `message` unless a message with the same id is already in the thread.
fn push_unique(thread: &mut Signal<Vec<Message>>, message: Message) {
    let mut list = thread.write();
    if !list.iter().any(|existing| existing.id == message.id) {
        list.push(message);
    }
}

/// Adds an incoming message and marks it unread if its sender isn't on screen.
fn absorb(
    thread: &mut Signal<Vec<Message>>,
    unread: &mut Signal<HashMap<UserId, u32>>,
    selected: Option<UserId>,
    message: Message,
) {
    let from = message.from;
    push_unique(thread, message);

    if selected != Some(from) {
        *unread.write().entry(from).or_insert(0) += 1;
    }
}

fn last_message(thread: &[Message], user: UserId) -> Option<String> {
    thread
        .iter()
        .rev()
        .find(|message| message.from == user || message.to == user)
        .map(|message| message.contents.clone())
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|word| word.chars().next())
        .collect::<String>()
        .to_uppercase()
}

fn relative_time(sent: u64) -> String {
    let diff = now_secs().saturating_sub(sent);
    match diff {
        0..=59 => "now".to_string(),
        60..=3599 => format!("{}m", diff / 60),
        3600..=86_399 => format!("{}h", diff / 3600),
        _ => format!("{}d", diff / 86_400),
    }
}

fn now_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
    }
}
