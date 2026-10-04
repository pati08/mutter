//! Sign-in page.

use dioxus::prelude::*;

use crate::api::auth::login;
use crate::routes::Route;
use crate::views::Brand;

#[component]
pub fn Login() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let navigator = use_navigator();

    let on_submit = move |event: FormEvent| async move {
        event.prevent_default();
        error.set(None);

        match login(username(), password()).await {
            Ok(Some(user)) => {
                let target = if user.must_change_password {
                    Route::ChangePassword {}
                } else if user.theme_color.is_none() {
                    Route::ThemePicker {}
                } else {
                    Route::Home {}
                };
                let _ = navigator.replace(target);
            }
            Ok(None) => error.set(Some("Incorrect username or password.".to_string())),
            Err(err) => error.set(Some(err.to_string())),
        }
    };

    rsx! {
        div { class: "flex min-h-dvh items-center justify-center accent-softer p-4",
            form {
                class: "accent-card w-full max-w-sm space-y-4 rounded-md p-8",
                onsubmit: on_submit,

                h1 { class: "text-slate-900",
                    Brand { size_class: "text-2xl" }
                }
                p { class: "text-sm text-slate-500", "Sign in to continue." }

                div { class: "space-y-1",
                    label { class: "block text-sm font-medium text-slate-700", r#for: "username", "Username" }
                    input {
                        id: "username",
                        r#type: "text",
                        autocomplete: "username",
                        required: true,
                        class: "accent-field w-full rounded-md px-3 py-2 placeholder:text-slate-400",
                        value: "{username}",
                        oninput: move |event| username.set(event.value()),
                    }
                }

                div { class: "space-y-1",
                    label { class: "block text-sm font-medium text-slate-700", r#for: "password", "Password" }
                    input {
                        id: "password",
                        r#type: "password",
                        autocomplete: "current-password",
                        required: true,
                        class: "accent-field w-full rounded-md px-3 py-2 placeholder:text-slate-400",
                        value: "{password}",
                        oninput: move |event| password.set(event.value()),
                    }
                }

                if let Some(message) = error() {
                    p { class: "text-sm text-red-600", "{message}" }
                }

                button {
                    r#type: "submit",
                    class: "accent-button w-full rounded-md px-4 py-2 font-medium",
                    "Sign in"
                }
            }
        }
    }
}
