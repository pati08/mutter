//! The chat screen: contact list, message thread, and composer.
//!
//! The thread lives only in memory for the session; a reload clears it. Incoming
//! messages are deduped by id and acknowledged in batches (a replay burst is acked
//! together). See `PLAN.md` M3–M4.

use std::collections::HashMap;

use dioxus::prelude::*;
use futures::FutureExt;

use crate::api::auth::logout;
use crate::api::chat::{ack_messages, contacts, events, send_message};
use crate::models::{Message, User, UserId};
use crate::routes::Route;
use crate::theme;
use crate::views::{Brand, RequireAuth};

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
    let mut search = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut connected = use_signal(|| true);
    // Bumped to restart the SSE stream after a disconnection.
    let mut generation = use_signal(|| 0u32);
    let contact_list = use_resource(move || async move { contacts().await });

    // Consume the SSE stream, on the client only. Reading `generation` subscribes
    // the effect to it, so the Reconnect button restarts the stream.
    use_effect(move || {
        let my_generation = generation();

        spawn(async move {
            connected.set(true);

            let mut stream = match events().await {
                Ok(stream) => stream,
                Err(err) => {
                    error.set(Some(err.to_string()));
                    connected.set(false);
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

            // The stream ended: replaced by another tab, or the network dropped.
            // Ignore it if this task has already been superseded by a reconnect.
            if *generation.peek() == my_generation {
                connected.set(false);
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

    let on_reconnect = move |_| {
        *generation.write() += 1;
    };

    // Read reactively (not `.peek()`) so changing the selection re-renders — the
    // back button and contact taps both rely on this.
    let selected_id = selected();
    let thread_snapshot = thread();
    let query = search().trim().to_lowercase();

    let contacts_state = contact_list();
    let selected_user = match &contacts_state {
        Some(Ok(list)) => {
            selected_id.and_then(|id| list.iter().find(|user| user.id == id).cloned())
        }
        _ => None,
    };
    let contacts_loading = contacts_state.is_none();
    let contacts_error = match &contacts_state {
        Some(Err(err)) => Some(err.to_string()),
        _ => None,
    };
    let visible_contacts: Vec<User> = match &contacts_state {
        Some(Ok(list)) => list
            .iter()
            .filter(|user| matches_search(user, &query))
            .cloned()
            .collect(),
        _ => Vec::new(),
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

    let selected_avatar_style = selected_user
        .as_ref()
        .and_then(|user| user.theme_color.as_deref())
        .map(theme::style_for)
        .unwrap_or_default();

    // On small screens show either the list or the thread, never both.
    let list_class = if selected_id.is_some() {
        "hidden min-h-0 w-full shrink-0 flex-col border-r border-slate-200 bg-white md:flex md:w-80"
    } else {
        "flex min-h-0 w-full shrink-0 flex-col border-r border-slate-200 bg-white md:w-80"
    };
    let thread_class = if selected_id.is_some() {
        "flex min-h-0 flex-1 flex-col"
    } else {
        "hidden min-h-0 flex-1 flex-col md:flex"
    };

    rsx! {
        div { class: "accent-softer flex h-dvh flex-col text-slate-900",
            if !connected() {
                div { class: "flex items-center justify-between gap-3 bg-amber-50 px-6 py-2 text-sm text-amber-800",
                    span { "Disconnected — this account may be open in another tab." }
                    button {
                        class: "accent-outline rounded-md px-2 py-0.5 font-medium",
                        onclick: on_reconnect,
                        "Reconnect"
                    }
                }
            }

            if let Some(message) = error() {
                div { class: "bg-red-50 px-6 py-2 text-sm text-red-700", "{message}" }
            }

            div { class: "flex min-h-0 flex-1",
                aside { class: "{list_class}",
                    header { class: "flex items-center justify-between px-5 py-4",
                        Brand { size_class: "text-xl" }
                        div { class: "flex items-center gap-2",
                            Link {
                                to: Route::ThemePicker {},
                                class: "accent-outline rounded-md px-2.5 py-1 text-xs font-medium",
                                "Theme"
                            }
                            SignOutButton {}
                        }
                    }

                    div { class: "px-4 pb-3",
                        input {
                            class: "accent-field w-full rounded-md px-4 py-2 text-sm placeholder:text-slate-400",
                            placeholder: "Search here...",
                            value: "{search}",
                            oninput: move |event| search.set(event.value()),
                        }
                    }

                    nav { class: "flex-1 space-y-1 overflow-y-auto px-2 pb-3",
                        if let Some(err) = contacts_error {
                            p { class: "px-3 py-3 text-sm text-red-600", "{err}" }
                        } else if contacts_loading {
                            p { class: "px-3 py-3 text-sm text-slate-400", "Loading contacts…" }
                        } else if visible_contacts.is_empty() {
                            p { class: "px-3 py-3 text-sm text-slate-400", "No conversations match." }
                        } else {
                            for user in visible_contacts {
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
                        }
                    }
                }

                main { class: "{thread_class}",
                    match selected_user {
                        Some(user) => rsx! {
                            header { class: "flex items-center gap-3 border-b border-slate-200 bg-white px-4 py-4 md:px-6",
                                button {
                                    class: "accent-outline rounded-md p-1.5 md:hidden",
                                    aria_label: "Back to conversations",
                                    onclick: move |_| selected.set(None),
                                    svg {
                                        class: "h-5 w-5",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        stroke: "currentColor",
                                        stroke_width: "2",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",
                                        path { d: "M15 18l-6-6 6-6" }
                                    }
                                }
                                div { class: "accent-fill flex h-10 w-10 items-center justify-center rounded text-sm font-semibold",
                                    style: "{selected_avatar_style}",
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
                                class: "flex items-center gap-3 border-t border-slate-200 bg-white px-4 py-4 md:px-6",
                                onsubmit: on_send,
                                input {
                                    class: "accent-field flex-1 rounded-md px-4 py-2 placeholder:text-slate-400",
                                    placeholder: "Type your message",
                                    value: "{draft}",
                                    oninput: move |event| draft.set(event.value()),
                                }
                                button {
                                    r#type: "submit",
                                    class: "accent-button rounded-md px-5 py-2 font-medium",
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
    let avatar_style = user
        .theme_color
        .as_deref()
        .map(theme::style_for)
        .unwrap_or_default();

    rsx! {
        button {
            class: if selected {
                "accent-soft flex w-full items-center gap-3 rounded-md px-3 py-2 text-left"
            } else {
                "accent-row flex w-full items-center gap-3 rounded-md px-3 py-2 text-left"
            },
            onclick: move |_| on_select.call(id),
            div { class: "accent-fill flex h-10 w-10 shrink-0 items-center justify-center rounded text-sm font-semibold",
                style: "{avatar_style}",
                "{initials(&user.name)}"
            }
            div { class: "min-w-0 flex-1",
                div { class: "truncate font-medium", "{user.name}" }
                div { class: "truncate text-sm text-slate-500", "{preview}" }
            }
            if unread > 0 {
                span { class: "accent-fill ml-2 rounded px-2 py-0.5 text-xs font-semibold",
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
                    "accent-fill max-w-md rounded-md px-4 py-2"
                } else {
                    "max-w-md rounded-md border border-slate-200 bg-white px-4 py-2 text-slate-800"
                },
                p { class: "whitespace-pre-wrap break-words", "{contents}" }
                div {
                    class: if mine {
                        "mt-1 text-right text-[11px] opacity-70"
                    } else {
                        "mt-1 text-right text-[11px] text-slate-400"
                    },
                    "{format_time(sent)}"
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
                div { class: "accent-soft mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-md",
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
            class: "accent-outline rounded-md px-2.5 py-1 text-xs font-medium",
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

fn matches_search(user: &User, query: &str) -> bool {
    query.is_empty()
        || user.name.to_lowercase().contains(query)
        || user.username.to_lowercase().contains(query)
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

/// Local wall-clock time as `HH:MM`.
fn format_time(sent: u64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        date.set_time((sent as f64) * 1000.0);
        format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = sent % 86_400;
        format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60)
    }
}
