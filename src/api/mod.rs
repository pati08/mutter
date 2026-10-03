//! Server functions shared between the client and the server.
//!
//! These compile on both sides: the client gets an HTTP stub, the server gets the
//! real handler. Server-only logic lives under `crate::server`.

pub mod admin;
pub mod auth;
pub mod chat;
