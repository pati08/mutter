//! Admin-only account management server functions.

use dioxus::prelude::*;

use crate::models::User;

#[cfg(feature = "server")]
use crate::server::auth as server_auth;
#[cfg(feature = "server")]
use tower_sessions::Session;

#[get("/api/admin/users", session: Session)]
pub async fn admin_list_users() -> ServerFnResult<Vec<User>> {
    Ok(server_auth::list_users(&session).await?)
}

/// Creates an account and returns its one-time temporary password.
#[post("/api/admin/users", session: Session)]
pub async fn admin_create_user(name: String, username: String) -> ServerFnResult<String> {
    Ok(server_auth::create_user(&session, &name, &username).await?)
}

/// Issues a fresh temporary password and re-arms the forced change.
#[post("/api/admin/reset-password", session: Session)]
pub async fn admin_reset_password(user_id: u32) -> ServerFnResult<String> {
    Ok(server_auth::reset_password(&session, crate::models::UserId(user_id)).await?)
}

#[post("/api/admin/delete-user", session: Session)]
pub async fn admin_delete_user(user_id: u32) -> ServerFnResult<()> {
    server_auth::delete_user(&session, crate::models::UserId(user_id)).await?;
    Ok(())
}
