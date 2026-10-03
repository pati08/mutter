//! mutter — a privacy-focused, queue-only chat relay.
//!
//! The server keeps a message only for as long as it takes to deliver it, and the
//! client keeps it only for the life of the page. See `PLAN.md` for the full design.

mod api;
mod app;
mod models;
mod routes;
mod views;

#[cfg(feature = "server")]
mod server;

fn main() {
    // On the server, build the SQLite pool, seed the admin, and add session
    // middleware before serving. Session and message state stay in memory.
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use tower_sessions::{MemoryStore, SessionManagerLayer};

        let pool = server::db::init().await?;

        // In development `serve` is re-invoked on hot reload; keep the first pool.
        let _ = server::db::DATABASE.try_set(pool);
        server::db::seed_admin_from_env().await?;

        let sessions = SessionManagerLayer::new(MemoryStore::default())
            // TODO: enable `Secure` cookies once served over TLS in production.
            .with_secure(false);

        Ok(dioxus::server::router(app::App).layer(sessions))
    });

    // The web/desktop/mobile client just boots the renderer.
    #[cfg(not(feature = "server"))]
    dioxus::launch(app::App);
}
