//! First-login theme color picker.

use dioxus::prelude::*;

use crate::api::auth::{me, set_theme_color};
use crate::routes::Route;
use crate::theme::{self, PRESETS};
use crate::views::RequireAuth;

#[component]
pub fn ThemePicker() -> Element {
    rsx! {
        RequireAuth { allow_missing_theme: true,
            ThemePickerInner {}
        }
    }
}

#[component]
fn ThemePickerInner() -> Element {
    let mut chosen = use_signal(|| theme::initial_accent(None));
    let mut seeded = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let navigator = use_navigator();
    let current = use_resource(move || async move { me().await });

    // Preselect the user's existing color once we know it; new users keep the
    // random preset chosen above.
    use_effect(move || {
        if seeded() {
            return;
        }

        if let Some(Ok(Some(user))) = current() {
            if let Some(color) = user.theme_color.as_deref() {
                chosen.set(color.to_string());
            }
            seeded.set(true);
        }
    });

    let on_save = move |_| async move {
        error.set(None);
        saving.set(true);

        match set_theme_color(chosen()).await {
            Ok(_) => {
                let _ = navigator.replace(Route::Home {});
            }
            Err(err) => {
                error.set(Some(err.to_string()));
                saving.set(false);
            }
        }
    };

    let selected = chosen();

    rsx! {
        div { class: "flex min-h-dvh items-center justify-center accent-softer p-4",
            div {
                class: "accent-card w-full max-w-lg space-y-6 rounded-md p-8",
                style: "{theme::style_for(&selected)}",
                div {
                    h1 { class: "text-2xl font-semibold text-slate-900", "Pick a theme color" }
                    p { class: "mt-1 text-sm text-slate-500",
                        "You can change this later from the sidebar."
                    }
                }

                div { class: "grid grid-cols-5 gap-3",
                    for preset in PRESETS {
                        button {
                            key: "{preset}",
                            class: if *preset == selected.as_str() {
                                "h-12 rounded-md ring-2 ring-slate-900 ring-offset-2"
                            } else {
                                "h-12 rounded-md ring-1 ring-slate-200 hover:ring-slate-400"
                            },
                            style: "background-color: {preset};",
                            aria_label: "Use {preset}",
                            onclick: move |_| chosen.set(preset.to_string()),
                        }
                    }
                }

                div { class: "flex items-center gap-3",
                    label { class: "text-sm font-medium text-slate-700", r#for: "custom-color", "Custom" }
                    input {
                        id: "custom-color",
                        r#type: "color",
                        class: "h-9 w-16 cursor-pointer rounded-md border border-slate-300 bg-white p-1",
                        value: "{selected}",
                        oninput: move |event| chosen.set(event.value()),
                    }
                    span { class: "font-mono text-sm text-slate-500", "{selected}" }
                }

                div { class: "rounded-md border border-slate-200 p-4",
                    div { class: "flex items-center gap-3",
                        div { class: "accent-fill font-cookie flex h-10 w-10 items-center justify-center rounded text-2xl leading-none",
                            "AB"
                        }
                        div { class: "accent-soft flex-1 rounded-md px-3 py-2 text-sm text-slate-700",
                            "Selected row"
                        }
                    }
                    div { class: "mt-3 flex items-center gap-3",
                        button { class: "accent-button rounded-md px-4 py-2 text-sm font-medium", "Button" }
                        button { class: "accent-outline rounded-md px-4 py-2 text-sm font-medium", "Secondary" }
                    }
                    div { class: "mt-3 flex justify-end",
                        div { class: "accent-fill max-w-xs rounded-md px-4 py-2 text-sm", "A themed bubble" }
                    }
                }

                if let Some(message) = error() {
                    p { class: "text-sm text-red-600", "{message}" }
                }

                button {
                    class: "accent-button w-full rounded-md px-4 py-2 font-medium disabled:opacity-60",
                    disabled: saving(),
                    onclick: on_save,
                    if saving() { "Saving…" } else { "Use this color" }
                }
            }
        }
    }
}
