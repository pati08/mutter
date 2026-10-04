//! The application's route table.

use dioxus::prelude::*;

use crate::views::{Admin, ChangePassword, Home, Login, ThemePicker};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/login")]
    Login {},
    #[route("/change-password")]
    ChangePassword {},
    #[route("/theme")]
    ThemePicker {},
    #[route("/admin")]
    Admin {},
    #[route("/")]
    Home {},
}
