//! Live SSE connections, at most one per user.
//!
//! Connecting replaces any previous sender, which ends the older stream. That gives
//! us the single-active-connection behavior agreed in `PLAN.md` §2.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use futures::channel::mpsc::UnboundedSender;

use crate::models::{Message, UserId};

/// Registry of the sender half of each user's SSE channel.
#[derive(Default)]
pub struct ConnectedUsers {
    senders: Mutex<HashMap<UserId, UnboundedSender<Message>>>,
}

impl ConnectedUsers {
    /// Registers a connection, replacing and thus closing any previous one.
    pub fn connect(&self, user: UserId, sender: UnboundedSender<Message>) {
        self.senders.lock().unwrap().insert(user, sender);
    }

    /// Best-effort push to a live connection. Returns whether it was sent. A send
    /// that fails means the receiver is gone, so the stale connection is forgotten.
    pub fn push(&self, user: UserId, message: Message) -> bool {
        let mut senders = self.senders.lock().unwrap();

        let Some(sender) = senders.get(&user) else {
            return false;
        };

        if sender.unbounded_send(message).is_ok() {
            return true;
        }

        senders.remove(&user);
        false
    }

    /// Drops a user's connection entirely, e.g. when the account is deleted.
    pub fn disconnect_user(&self, user: UserId) {
        self.senders.lock().unwrap().remove(&user);
    }
}

/// The process-wide connection registry.
pub static CONNECTED_USERS: LazyLock<ConnectedUsers> = LazyLock::new(ConnectedUsers::default);
