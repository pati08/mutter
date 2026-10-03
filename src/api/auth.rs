//! Authentication server functions.

use dioxus::prelude::*;

use crate::models::User;

#[cfg(feature = "server")]
use crate::server::auth as server_auth;
#[cfg(feature = "server")]
use tower_sessions::Session;

/// Sign in. Returns `Ok(None)` when the credentials are wrong.
#[post("/api/auth/login", session: Session)]
pub async fn login(username: String, password: String) -> ServerFnResult<Option<User>> {
    Ok(server_auth::login(&session, &username, &password).await?)
}

/// The currently signed-in user, if any.
#[get("/api/auth/me", session: Session)]
pub async fn me() -> ServerFnResult<Option<User>> {
    Ok(server_auth::current_user(&session).await?)
}

/// End the current session.
#[post("/api/auth/logout", session: Session)]
pub async fn logout() -> ServerFnResult<()> {
    server_auth::logout(&session).await?;
    Ok(())
}

/// Change the signed-in user's password.
#[post("/api/auth/change-password", session: Session)]
pub async fn change_password(current: String, new: String) -> ServerFnResult<User> {
    Ok(server_auth::change_password(&session, &current, &new).await?)
}
