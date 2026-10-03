//! The core relay: send, acknowledge, and stream messages.
//!
//! A message is enqueued for its recipient and stays there until the recipient's
//! client acknowledges it. Nothing is persisted.

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::anyhow;
use dioxus::fullstack::ServerEvents;
use dioxus::server::axum::BoxError;
use futures::channel::mpsc;
use futures::StreamExt;
use tower_sessions::Session;

use crate::models::{Message, MessageId, UnixTimestamp, User, UserId};
use crate::server::auth::require_user;
use crate::server::connected_users::CONNECTED_USERS;
use crate::server::db;
use crate::server::queues::MESSAGE_QUEUES;

/// Monotonic message ids; they only need to be unique for the life of the process.
static NEXT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Everyone the signed-in user can message (currently all other users).
pub async fn contacts(session: &Session) -> anyhow::Result<Vec<User>> {
    let me = require_user(session).await?;
    let everyone = db::list_users().await?;

    Ok(everyone
        .into_iter()
        .filter(|user| user.id != me.id)
        .collect())
}

/// Enqueues a message for `to`, and pushes it over their live connection if any.
pub async fn send_message(
    session: &Session,
    to: UserId,
    contents: String,
) -> anyhow::Result<Message> {
    let from = require_user(session).await?.id;

    let contents = contents.trim().to_string();
    if contents.is_empty() {
        return Err(anyhow!("cannot send an empty message"));
    }
    if db::find_by_id(to).await?.is_none() {
        return Err(anyhow!("no such recipient"));
    }

    let message = Message {
        id: MessageId(NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed)),
        from,
        to,
        contents,
        sent: UnixTimestamp(current_timestamp()),
    };

    // Keep it until acknowledged, then try to deliver it right away.
    MESSAGE_QUEUES.enqueue(message.clone());
    let _ = CONNECTED_USERS.push(to, message.clone());

    Ok(message)
}

/// Drops acknowledged messages from the signed-in user's queue.
pub async fn ack_messages(session: &Session, ids: Vec<MessageId>) -> anyhow::Result<()> {
    let user = require_user(session).await?;
    MESSAGE_QUEUES.ack(user.id, &ids);
    Ok(())
}

/// Opens the signed-in user's SSE stream: replays unacknowledged messages, then
/// streams new ones until the connection closes. A new connection replaces (and
/// closes) any previous one.
pub async fn subscribe(session: &Session) -> anyhow::Result<ServerEvents<Message>> {
    let user = require_user(session).await?;

    let (sender, receiver) = mpsc::unbounded::<Message>();

    // Register first, then replay, so nothing new is missed.
    CONNECTED_USERS.connect(user.id, sender);

    for message in MESSAGE_QUEUES.pending(user.id) {
        if !CONNECTED_USERS.push(user.id, message) {
            break;
        }
    }

    Ok(ServerEvents::from_stream(
        receiver.map(Ok::<Message, BoxError>),
    ))
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
