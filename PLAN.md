# mutter — Project Plan

> A privacy-focused chat app. The server is a **relay, not a store**: messages live
> only in an in-memory queue and are removed once the recipient's client confirms
> delivery. Neither side keeps a message history beyond the current session.

This document records the agreed goal, the architectural decisions, and the roadmap.
It is the source of truth for scope. Anything not listed under "In scope" is
deliberately deferred.

---

## 1. Goal

- **Privacy first.** No message history is persisted anywhere. A message exists on
  the server only for as long as it takes to ensure it reached the recipient's client,
  and on the client only for the life of the page. A reload loses the thread.
- **Server as a queue.** Incoming messages for a user sit in a per-user in-memory
  queue. They are pushed over a live SSE stream and removed once acknowledged.
- **Client is session-only.** Received messages live in component state. There is no
  localStorage / IndexedDB / token-for-messages persistence.
- **Accounts are the exception.** Identity (name, username, password hash) *does*
  persist, because the app is invite-only and an admin has to be able to hand out
  accounts. This is the only durable state besides the user table.
- **Simple chat UI.** Two panes, per the `inso/` references: conversation list on the
  left, message thread on the right, sent bubbles right-aligned and colored, received
  bubbles left-aligned and neutral. Target: the minimal purple reference, light theme.

## 2. Decisions (locked)

| Area | Decision |
| --- | --- |
| Receive transport | **SSE server push**, using Dioxus fullstack's built-in `ServerEvents<T>` / `SseTx<T>` (`futures_channel::mpsc` under the hood). |
| Identity | **Username + password.** Invite-only: an admin is the only one who can create accounts. |
| Activation flow | Admin creates an account and gets a **one-time temporary password**. The user logs in with it and is **forced to change it** before doing anything else. |
| Account storage | **SQLite via `sqlx`.** Single file. Only the `users` table persists. |
| Theme | **Per-user accent stored on the account.** Chosen on first login from pastel presets or an advanced color picker, and applied through CSS custom properties with derived foreground, border, and soft shades. |
| Sessions | **`tower-sessions` with its in-memory store.** HttpOnly, `SameSite=Strict` cookie (name `id`); a server restart logs everyone out. |
| Server state | **`Lazy` / `LazyLock` globals** (Dioxus fullstack's recommended global state), rather than an axum `Extension` + `ServerState`. |
| Message privacy | **Plaintext in memory, never persisted.** Server can read contents while queued; nothing touches disk. |
| Delivery guarantee | **ACK-based redelivery.** A message stays queued until the client confirms it landed in its in-memory thread. Unacked messages are re-sent on reconnect. |
| Retention | **No cap** for offline recipients. Accepted risk; revisit if queues grow unbounded. |
| Multi-tab | **Single active connection per user.** A new SSE connection replaces the old one. |
| MVP features | **1:1 text messages only.** Groups, read receipts, typing indicators, attachments, and E2E are deferred (Section 8). |

### Why not the current scaffold's `std::sync::mpsc`

`std::sync::mpsc::Sender` is `Send` but **not `Sync`**, so it cannot live inside the
shared `ConnectedUsers` state that axum handlers touch. The SSE design uses
`futures_channel::mpsc::UnboundedSender<Message>` (which is `Send + Sync + Clone`),
matching what `ServerEvents` already uses. The existing `server/` modules are
rewritten accordingly.

## 3. Architecture

```text
                      ┌──────────────────────── server ────────────────────────┐
 sender browser       │                                                        │  recipient browser
┌────────────┐  POST  │  ┌──────────┐   enqueue    ┌──────────────────────┐    │  GET /api/events (SSE)
│ send msg   │────────┼─▶│ auth/send│─────────────▶│ queue[recipient_id]  │    │◀────────────────────────
└────────────┘        │  └──────────┘              │  VecDeque<Message>   │    │   data: {Message}
                      │        │                   └──────────┬───────────┘    │
                      │        │ if online: push             │ SSE push       │
                      │        └─────────────────────────────┼───────────────▶│   add to in-memory thread
                      │                                      │                │
                      │        POST /api/ack {ids}  ┌────────▼───────────┐    │───────────────┐
                      │◀────────────────────────────│ remove acked ids   │◀───┼── ack(msg.id) │
                      │                             └────────────────────┘    │               │
                      └────────────────────────────────────────────────────────┘
```

Key points:

- **Enqueue happens on send, always.** Even if the recipient is offline. The queue is
  the source of truth for "not yet delivered".
- **Push is best-effort.** If the recipient has an active SSE sender, the message is
  also pushed immediately. Delivery is *not* considered complete until the ack.
- **Ack is batched.** The client appends messages to its thread and acks the ids,
  debounced (e.g. one ack per animation frame / short interval) to avoid a request
  per message.
- **Reconnect replays.** When a user reconnects (or reloads), the SSE endpoint
  re-sends the entire unacked queue before streaming new items. This is how a page
  refresh restores messages the user hadn't confirmed — the only "recovery" window.

### Single active connection

`ConnectedUsers` maps `UserId -> UnboundedSender<Message>`. When `events()` runs, it
creates a fresh channel and **replaces** the stored sender; dropping the old sender
ends the old stream and closes the previous tab's connection. The client does **not**
auto-reconnect after a stream ends, to avoid two tabs fighting (each reconnect would
kick the other). The old tab shows a "disconnected elsewhere" state.

## 4. Data model

```rust
// Shared (serializable across client/server)
pub struct UserId(u32);

pub struct User {
    pub id: UserId,
    pub name: String,
    pub username: String,
    pub is_admin: bool,
    pub must_change_password: bool,
}                          // never includes the password hash

pub struct Message {
    pub id: MessageId,      // u64 or UUID; used for acking
    pub from: UserId,
    pub to: UserId,
    pub contents: String,
    pub sent: UnixTimestamp,
}
```

SQLite (`users` table only):

```sql
CREATE TABLE users (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT    NOT NULL,
    username             TEXT    NOT NULL UNIQUE,
    password_hash        TEXT    NOT NULL,
    is_admin             INTEGER NOT NULL DEFAULT 0,
    must_change_password INTEGER NOT NULL DEFAULT 1,
    created_at           INTEGER NOT NULL
);
```

In memory (lost on restart, by design):

- `MessageQueues`: `HashMap<UserId, VecDeque<Message>>`
- `ConnectedUsers`: `HashMap<UserId, UnboundedSender<Message>>`
- `Sessions`: `HashMap<SessionToken, UserId>` (HTTP-only cookie holds the token)

## 5. Server API

Dioxus server functions (`#[get]` / `#[post]`), all returning `Result<T>`.

### Auth

- `login(username, password) -> Result<User>` — sets session cookie
- `logout() -> Result<()>`
- `change_password(current, new) -> Result<()>`
- `me() -> Result<User>`

### Admin (admin-only)

- `admin_list_users() -> Result<Vec<User>>`
- `admin_create_user(name, username) -> Result<String>` — returns the temp password once
- `admin_reset_password(user_id) -> Result<String>` — new temp password
- `admin_delete_user(user_id) -> Result<()>`

### Chat

- `contacts() -> Result<Vec<User>>` — all other users (no separate contact model yet)
- `send_message(to, contents) -> Result<Message>` — enqueues + best-effort push
- `ack_messages(ids: Vec<MessageId>) -> Result<()>` — removes from the recipient's queue
- `events() -> Result<ServerEvents<Message>>` — the SSE stream, gated by session auth

## 6. Auth & admin flow

1. **Bootstrap.** On startup, if no admin exists, create one from
   `MUTTER_ADMIN_USERNAME` / `MUTTER_ADMIN_PASSWORD` env vars.
2. **Create.** Admin opens `/admin`, enters a name + username. Server generates a
   random temp password, hashes it, stores `must_change_password = 1`, and shows the
   temp password **once** to the admin to hand off.
3. **Activate.** User logs in with the temp password. Server redirects them to
   `/change-password` and blocks every other route until `must_change_password = 0`.
4. **Reset.** `admin_reset_password` generates a new temp password and re-arms the
   forced change. Deleting a user also drops their queue and live connection.
5. **Sessions.** `tower-sessions` keeps session records in an in-memory store and
   issues an `id` cookie (HttpOnly, `SameSite=Strict`). Logging in cycles the id to
   guard against fixation; a server restart logs everyone out.

Password hashing: **Argon2**. Temp passwords: random, shown once, never recoverable.

## 7. UI

Routes (Dioxus router):

- `/login` — username + password
- `/change-password` — forced when `must_change_password`
- `/theme` — accent picker (on first login, or from the sidebar/admin header)
- `/` — chat (auth required)
- `/admin` — user management (admin only)

Chat screen follows the minimal `inso/chat-ui-3.webp` layout, but with slightly
rounded rectangles rather than pills/circles:

- **Left pane:** conversation list (rounded-square avatar in the contact's chosen
  color, name, last-message preview, unread badge).
- **Right pane:** message thread (sent bubbles in the viewer's accent, received
  bubbles neutral, local `HH:MM` times) and a bottom composer.
- In-memory thread state lives in a `Signal<Vec<Message>>`; cleared on reload.

The accent is the account's theme color. `RequireAuth` publishes it as CSS custom
properties (`--accent`, `--accent-fg`, `--accent-border`, `--accent-soft`,
`--accent-softer`), and the `.accent-*` helpers in `assets/main.css` consume them, so
the palette changes at runtime without rebuilding Tailwind. Each contact's avatar
uses their own stored color; the signed-out screens use the `:root` default.

## 8. Deferred / out of scope for v1

Recorded here so they are not lost:

- **Group chats** — fan-out to multiple queues, per-message sender identity.
- **Read receipts / delivery ticks** — needs an ack path back to the sender.
- **Typing indicators** — an ephemeral second message type.
- **E2E encryption** — server currently sees plaintext while queued.
- **Metadata minimization** (padding/timing defenses).
- **Attachments / images / voice** — the references show these; not now.
- **Message retention controls** — no TTL today (accepted risk).
- **Multi-device per user** — single active connection for now.
- **Desktop / mobile polish** — features exist in `Cargo.toml` but are untested.

## 9. Roadmap

- **M0 — Foundation. ✅ Done.** Split `main.rs` into `app.rs`, `routes`, `models`,
  `views`, and a real `server/` module; add dependencies (`sqlx`, `argon2`, `rand`,
  `tower-sessions`, `futures`); both feature builds and clippy clean.
- **M1 — Accounts & auth. ✅ Done.** SQLite schema + `sqlx` pool; Argon2 hashing;
  admin bootstrap from env; `tower-sessions` login/logout; forced password change;
  admin UI and server functions. Verified end-to-end over HTTP.
- **M2 — Core relay. ✅ Done.** `MessageQueues`, `ConnectedUsers`, `send_message`,
  `ack_messages`, and the `events()` SSE endpoint with replay of unacknowledged
  messages on connect. Verified over HTTP: live delivery, ack removal, offline
  replay, single-active-connection, and an auth-gated stream. A reconnecting client
  may receive a replay duplicate, so the M3 client dedupes by `MessageId`.
- **M3 — Chat UI. ✅ Done.** Two-pane screen (`src/views/chat.rs`): contact list with
  last-message previews and unread badges, message thread with sent/received bubbles
  and relative timestamps, composer, live SSE consumption with dedupe by `MessageId`,
  and batched acks (a replay burst is acked together). `dx build --platform web`
  succeeds; native, server, and wasm builds clean; SSR renders `/login` and `/`.
  Browser interaction itself was not automated.
- **M4 — Polish. ✅ Done.** Contact **search** filters by name/username;
  **disconnection handling** shows a banner with a Reconnect button when the SSE
  stream ends (e.g. replaced by another tab); **responsive layout** shows the list or
  the thread on small screens (with a back button) and both on `md+`; timestamps are
  now local `HH:MM`. Native, server, wasm, and clippy all clean; `dx build` emits the
  responsive utilities. Browser interaction is still not automated.

## 10. Assumptions to confirm

- Admin is bootstrapped from environment variables (vs. a first-run setup page).
- Session tokens are in-memory; a server restart logs everyone out.
- The client does not auto-reconnect an SSE stream that was replaced by a newer tab.
- `contacts()` is "all other users" — there is no friend/contact relationship yet.
- "No cap" retention means a dead account's queue grows until the recipient connects
  or is deleted; we will monitor and can add a TTL later without changing the API.

## 11. Reference material

- UI inspiration images: `inso/chat-ui-1.png`, `inso/chat-ui-2.png`,
  `inso/chat-ui-3.webp` (target: #3).
- Dioxus fullstack SSE: `dioxus_fullstack` `ServerEvents<T>` / `SseTx<T>`
  (docs: *Streams and SSE*). Generic streams: `Streaming<T>`, `TextStream`.
- Dioxus auth guidance: session middleware + extractors (no built-in auth).
