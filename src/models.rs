//! Types shared between the client and the server.

use serde::{Deserialize, Serialize};

/// Stable identifier for a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub u32);

/// A user as seen by the client. Never carries credential material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub username: String,
    pub is_admin: bool,
    pub must_change_password: bool,
    /// The user's chosen accent color, if they have picked one yet.
    pub theme_color: Option<String>,
}

/// Identifier for a single message, used for acknowledging delivery.
///
/// Only constructed on the server, so the client build reports it as unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

/// A chat message in transit.
///
/// Only constructed on the server, so the client build reports it as unused.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub from: UserId,
    pub to: UserId,
    pub contents: String,
    pub sent: UnixTimestamp,
}

/// Seconds since the Unix epoch.
///
/// Only constructed on the server, so the client build reports it as unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixTimestamp(pub u64);
