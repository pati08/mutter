//! Auth gating for protected pages.

use dioxus::prelude::*;

use crate::api::auth::me;
use crate::routes::Route;

/// Renders `children` only for a signed-in user. Redirects to `/login` when signed
/// out, to `/change-password` when a change is pending, and home for non-admins when
/// `admin_only` is set.
#[component]
pub fn RequireAuth(#[props(default)] admin_only: bool, children: Element) -> Element {
    let current = use_resource(move || async move { me().await });
    let navigator = use_navigator();

    use_effect(move || {
        let Some(result) = current() else {
            return;
        };

        let redirect = match result {
            Ok(Some(user)) if user.must_change_password => Some(Route::ChangePassword {}),
            Ok(Some(user)) if admin_only && !user.is_admin => Some(Route::Home {}),
            Ok(Some(_)) => None,
            _ => Some(Route::Login {}),
        };

        if let Some(target) = redirect {
            let _ = navigator.replace(target);
        }
    });

    match current() {
        Some(Ok(Some(user))) if !user.must_change_password && (!admin_only || user.is_admin) => {
            rsx! { {children} }
        }
        _ => rsx! {
            div { class: "flex min-h-dvh items-center justify-center text-slate-400", "Loading…" }
        },
    }
}
