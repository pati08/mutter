//! Server-only state and helpers.
//!
//! Nothing here is compiled on the client. Queues and connections are in-memory by
//! design; only the user table is durable. See `PLAN.md` §1 and §3.

pub mod auth;
pub mod chat;
pub mod connected_users;
pub mod db;
pub mod queues;
