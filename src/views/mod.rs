//! Page components.

mod admin;
mod change_password;
mod chat;
mod login;
mod require_auth;

pub use admin::Admin;
pub use change_password::ChangePassword;
pub use chat::Home;
pub use login::Login;
pub use require_auth::RequireAuth;
