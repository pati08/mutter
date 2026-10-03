//! Per-user in-memory message queues.
//!
//! A message lives here only until the recipient's client acknowledges it, and it is
//! never written to disk. See `PLAN.md` §1 and §3.

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use crate::models::{Message, MessageId, UserId};

/// All queues, keyed by recipient.
#[derive(Default)]
pub struct MessageQueues {
    queues: Mutex<HashMap<UserId, VecDeque<Message>>>,
}

impl MessageQueues {
    /// Adds a message to its recipient's queue.
    pub fn enqueue(&self, message: Message) {
        self.queues
            .lock()
            .unwrap()
            .entry(message.to)
            .or_default()
            .push_back(message);
    }

    /// Drops acknowledged messages from `user`'s queue, returning how many were removed.
    pub fn ack(&self, user: UserId, ids: &[MessageId]) -> usize {
        let mut queues = self.queues.lock().unwrap();
        let Some(queue) = queues.get_mut(&user) else {
            return 0;
        };

        let before = queue.len();
        queue.retain(|message| !ids.contains(&message.id));
        before - queue.len()
    }

    /// Everything still waiting for `user`, oldest first.
    pub fn pending(&self, user: UserId) -> Vec<Message> {
        self.queues
            .lock()
            .unwrap()
            .get(&user)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Forgets a user's queue entirely, e.g. when the account is deleted.
    pub fn remove_user(&self, user: UserId) {
        self.queues.lock().unwrap().remove(&user);
    }
}

/// The process-wide queue registry.
pub static MESSAGE_QUEUES: LazyLock<MessageQueues> = LazyLock::new(MessageQueues::default);
