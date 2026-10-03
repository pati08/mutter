//! Chat server functions: contacts, send, acknowledge, and the SSE event stream.

use dioxus::prelude::*;

use crate::models::{Message, MessageId, User, UserId};

#[cfg(feature = "server")]
use crate::server::chat as server_chat;
#[cfg(feature = "server")]
use tower_sessions::Session;

/// Everyone the signed-in user can message (currently all other users).
#[get("/api/contacts", session: Session)]
pub async fn contacts() -> ServerFnResult<Vec<User>> {
    Ok(server_chat::contacts(&session).await?)
}

/// Sends a 1:1 message. It stays queued until the recipient acknowledges it.
#[post("/api/messages", session: Session)]
pub async fn send_message(to: UserId, contents: String) -> ServerFnResult<Message> {
    Ok(server_chat::send_message(&session, to, contents).await?)
}

/// Acknowledges delivered messages so the server can drop them.
#[post("/api/messages/ack", session: Session)]
pub async fn ack_messages(ids: Vec<MessageId>) -> ServerFnResult<()> {
    server_chat::ack_messages(&session, ids).await?;
    Ok(())
}

/// The live SSE stream of incoming messages.
#[get("/api/events", session: Session)]
pub async fn events() -> ServerFnResult<dioxus::fullstack::ServerEvents<Message>> {
    Ok(server_chat::subscribe(&session).await?)
}
