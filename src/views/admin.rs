//! Admin account management.

use dioxus::prelude::*;

use crate::api::admin::{
    admin_create_user, admin_delete_user, admin_list_users, admin_reset_password,
};
use crate::api::auth::logout;
use crate::routes::Route;
use crate::views::RequireAuth;

#[component]
pub fn Admin() -> Element {
    rsx! {
        RequireAuth { admin_only: true,
            AdminPanel {}
        }
    }
}

#[component]
fn AdminPanel() -> Element {
    let users = use_resource(move || async move { admin_list_users().await });
    let mut name = use_signal(String::new);
    let mut username = use_signal(String::new);
    let mut status = use_signal(|| None::<String>);
    let mut error = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let on_create = move |event: FormEvent| async move {
        event.prevent_default();
        error.set(None);

        match admin_create_user(name(), username()).await {
            Ok(temp_password) => {
                status.set(Some(format!(
                    "Temporary password for \"{}\": {temp_password}",
                    username()
                )));
                name.set(String::new());
                username.set(String::new());
                let mut resource = users;
                resource.restart();
            }
            Err(err) => error.set(Some(err.to_string())),
        }
    };

    rsx! {
        div { class: "min-h-dvh bg-slate-50",
            header { class: "flex items-center justify-between border-b border-slate-200 bg-white px-6 py-3",
                span { class: "text-lg font-semibold text-slate-900", "mutter · accounts" }
                button {
                    class: "rounded-lg border border-slate-300 px-3 py-1.5 text-sm font-medium text-slate-700 hover:bg-slate-100",
                    onclick: move |_| async move {
                        let _ = logout().await;
                        let _ = navigator.replace(Route::Login {});
                    },
                    "Sign out"
                }
            }

            main { class: "mx-auto max-w-3xl space-y-6 p-6",
                form {
                    class: "flex flex-wrap items-end gap-3 rounded-2xl bg-white p-5 shadow",
                    onsubmit: on_create,

                    div { class: "flex-1 space-y-1",
                        label { class: "block text-sm font-medium text-slate-700", r#for: "name", "Full name" }
                        input {
                            id: "name",
                            r#type: "text",
                            required: true,
                            class: "w-full rounded-lg border border-slate-300 px-3 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-500",
                            value: "{name}",
                            oninput: move |event| name.set(event.value()),
                        }
                    }

                    div { class: "flex-1 space-y-1",
                        label { class: "block text-sm font-medium text-slate-700", r#for: "new-username", "Username" }
                        input {
                            id: "new-username",
                            r#type: "text",
                            required: true,
                            class: "w-full rounded-lg border border-slate-300 px-3 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-500",
                            value: "{username}",
                            oninput: move |event| username.set(event.value()),
                        }
                    }

                    button {
                        r#type: "submit",
                        class: "rounded-lg bg-indigo-600 px-4 py-2 font-medium text-white hover:bg-indigo-700",
                        "Create account"
                    }
                }

                if let Some(message) = status() {
                    div { class: "rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800",
                        "{message}"
                    }
                }
                if let Some(message) = error() {
                    div { class: "rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800",
                        "{message}"
                    }
                }

                match users() {
                    Some(Ok(list)) => rsx! {
                        div { class: "overflow-hidden rounded-2xl bg-white shadow",
                            table { class: "w-full text-left",
                                thead { class: "bg-slate-100 text-xs uppercase text-slate-500",
                                    tr {
                                        th { class: "px-4 py-2", "User" }
                                        th { class: "px-4 py-2", "Role" }
                                        th { class: "px-4 py-2", "Status" }
                                        th { class: "px-4 py-2" }
                                    }
                                }
                                tbody {
                                    for user in list {
                                        tr { key: "{user.id.0}", class: "border-t border-slate-100",
                                            td { class: "px-4 py-3",
                                                div { class: "font-medium text-slate-800", "{user.name}" }
                                                div { class: "text-sm text-slate-500", "@{user.username}" }
                                            }
                                            td { class: "px-4 py-3 text-sm",
                                                if user.is_admin {
                                                    span { class: "rounded bg-indigo-100 px-2 py-0.5 text-indigo-700", "admin" }
                                                } else {
                                                    span { class: "text-slate-600", "user" }
                                                }
                                            }
                                            td { class: "px-4 py-3 text-sm",
                                                if user.must_change_password {
                                                    span { class: "text-amber-600", "pending" }
                                                } else {
                                                    span { class: "text-emerald-600", "active" }
                                                }
                                            }
                                            td { class: "px-4 py-3 text-right text-sm",
                                                button {
                                                    class: "mr-2 rounded border border-slate-300 px-2 py-1 text-slate-700 hover:bg-slate-100",
                                                    onclick: move |_| async move {
                                                        error.set(None);
                                                        match admin_reset_password(user.id.0).await {
                                                            Ok(temp) => status.set(Some(format!("New temporary password: {temp}"))),
                                                            Err(err) => error.set(Some(err.to_string())),
                                                        }
                                                    },
                                                    "Reset"
                                                }
                                                button {
                                                    class: "rounded border border-red-200 px-2 py-1 text-red-600 hover:bg-red-50",
                                                    onclick: move |_| async move {
                                                        error.set(None);
                                                        match admin_delete_user(user.id.0).await {
                                                            Ok(()) => {
                                                                let mut resource = users;
                                                                resource.restart();
                                                            }
                                                            Err(err) => error.set(Some(err.to_string())),
                                                        }
                                                    },
                                                    "Delete"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(err)) => rsx! {
                        p { class: "text-sm text-red-600", "Could not load users: {err}" }
                    },
                    None => rsx! {
                        p { class: "text-sm text-slate-400", "Loading users…" }
                    },
                }
            }
        }
    }
}
