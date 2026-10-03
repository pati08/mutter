//! Forced password change after a temporary password is issued.

use dioxus::prelude::*;

use crate::api::auth::change_password;
use crate::routes::Route;

#[component]
pub fn ChangePassword() -> Element {
    let mut current = use_signal(String::new);
    let mut new = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let on_submit = move |event: FormEvent| async move {
        event.prevent_default();
        error.set(None);

        if new() != confirm() {
            error.set(Some("The new passwords do not match.".to_string()));
            return;
        }

        match change_password(current(), new()).await {
            Ok(_) => {
                let _ = navigator.replace(Route::Home {});
            }
            Err(err) => error.set(Some(err.to_string())),
        }
    };

    rsx! {
        div { class: "flex min-h-dvh items-center justify-center bg-slate-100 p-4",
            form {
                class: "w-full max-w-sm space-y-4 rounded-2xl bg-white p-8 shadow-lg",
                onsubmit: on_submit,

                h1 { class: "text-2xl font-semibold text-slate-900", "Choose a new password" }
                p { class: "text-sm text-slate-500",
                    "Your account was created with a temporary password. Set your own to continue."
                }

                div { class: "space-y-1",
                    label { class: "block text-sm font-medium text-slate-700", r#for: "current", "Temporary password" }
                    input {
                        id: "current",
                        r#type: "password",
                        autocomplete: "current-password",
                        required: true,
                        class: "w-full rounded-lg border border-slate-300 px-3 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-500",
                        value: "{current}",
                        oninput: move |event| current.set(event.value()),
                    }
                }

                div { class: "space-y-1",
                    label { class: "block text-sm font-medium text-slate-700", r#for: "new", "New password" }
                    input {
                        id: "new",
                        r#type: "password",
                        autocomplete: "new-password",
                        required: true,
                        class: "w-full rounded-lg border border-slate-300 px-3 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-500",
                        value: "{new}",
                        oninput: move |event| new.set(event.value()),
                    }
                }

                div { class: "space-y-1",
                    label { class: "block text-sm font-medium text-slate-700", r#for: "confirm", "Confirm new password" }
                    input {
                        id: "confirm",
                        r#type: "password",
                        autocomplete: "new-password",
                        required: true,
                        class: "w-full rounded-lg border border-slate-300 px-3 py-2 text-slate-900 placeholder:text-slate-400 outline-none focus:border-indigo-500",
                        value: "{confirm}",
                        oninput: move |event| confirm.set(event.value()),
                    }
                }

                if let Some(message) = error() {
                    p { class: "text-sm text-red-600", "{message}" }
                }

                button {
                    r#type: "submit",
                    class: "w-full rounded-lg bg-indigo-600 px-4 py-2 font-medium text-white transition hover:bg-indigo-700",
                    "Save password"
                }
            }
        }
    }
}
