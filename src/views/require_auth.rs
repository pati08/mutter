//! Auth gating for protected pages.

use dioxus::prelude::*;

use crate::api::auth::me;
use crate::routes::Route;
use crate::theme;

/// Renders `children` only for a signed-in user. Redirects to `/login` when signed
/// out, to `/change-password` when a change is pending, and to `/theme` when the
/// user hasn't chosen an accent yet (unless `allow_missing_theme`). With
/// `admin_only`, non-admins are sent home.
///
/// The current user's accent is applied to `children` through CSS variables.
#[component]
pub fn RequireAuth(
    #[props(default)] admin_only: bool,
    #[props(default)] allow_missing_theme: bool,
    children: Element,
) -> Element {
    let current = use_resource(move || async move { me().await });
    let navigator = use_navigator();

    use_effect(move || {
        let Some(result) = current() else {
            return;
        };

        let redirect = match result {
            Ok(Some(user)) if user.must_change_password => Some(Route::ChangePassword {}),
            Ok(Some(user)) if !allow_missing_theme && user.theme_color.is_none() => {
                Some(Route::ThemePicker {})
            }
            Ok(Some(user)) if admin_only && !user.is_admin => Some(Route::Home {}),
            Ok(Some(_)) => None,
            _ => Some(Route::Login {}),
        };

        if let Some(target) = redirect {
            let _ = navigator.replace(target);
        }
    });

    match current() {
        Some(Ok(Some(user)))
            if !user.must_change_password
                && (allow_missing_theme || user.theme_color.is_some())
                && (!admin_only || user.is_admin) =>
        {
            let style =
                theme::style_for(user.theme_color.as_deref().unwrap_or(theme::DEFAULT_ACCENT));

            rsx! {
                div { style: "{style} display: contents;", {children} }
            }
        }
        _ => rsx! {
            div { class: "flex min-h-dvh items-center justify-center text-slate-400", "Loading…" }
        },
    }
}
