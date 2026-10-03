//! SQLite persistence for accounts.
//!
//! This is the only durable state in the app. Messages never touch the database.

use anyhow::Context;
use dioxus::fullstack::Lazy;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

use crate::models::{User, UserId};

/// The process-wide connection pool. Initialized in `main` before serving.
pub static DATABASE: Lazy<SqlitePool> = Lazy::lazy();

fn pool() -> &'static SqlitePool {
    &DATABASE
}

/// Opens the database and creates the schema if needed.
pub async fn init() -> anyhow::Result<SqlitePool> {
    let path = std::env::var("MUTTER_DB").unwrap_or_else(|_| "mutter.db".to_string());

    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .context("could not open the mutter database")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id                   INTEGER PRIMARY KEY AUTOINCREMENT,
            name                 TEXT    NOT NULL,
            username             TEXT    NOT NULL UNIQUE,
            password_hash        TEXT    NOT NULL,
            is_admin             INTEGER NOT NULL DEFAULT 0,
            must_change_password INTEGER NOT NULL DEFAULT 1,
            created_at           INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .context("could not create the users table")?;

    Ok(pool)
}

/// Creates the initial admin from the environment if no admin exists yet.
pub async fn seed_admin_from_env() -> anyhow::Result<()> {
    if admin_count().await? > 0 {
        return Ok(());
    }

    let username = std::env::var("MUTTER_ADMIN_USERNAME");
    let password = std::env::var("MUTTER_ADMIN_PASSWORD");

    match (username, password) {
        (Ok(username), Ok(password)) if !username.is_empty() && !password.is_empty() => {
            let hash = super::auth::hash_password(&password)?;
            insert_user("Admin", &username, &hash, true, false).await?;
            eprintln!("mutter: created admin account `{username}`");
        }
        _ => {
            eprintln!(
                "mutter: no admin exists. Set MUTTER_ADMIN_USERNAME and MUTTER_ADMIN_PASSWORD to create one."
            );
        }
    }

    Ok(())
}

/// A user row including the password hash. Server-side only.
pub struct StoredUser {
    pub id: UserId,
    pub name: String,
    pub username: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub must_change_password: bool,
}

impl StoredUser {
    pub fn into_user(self) -> User {
        User {
            id: self.id,
            name: self.name,
            username: self.username,
            is_admin: self.is_admin,
            must_change_password: self.must_change_password,
        }
    }

    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: UserId(row.try_get::<i64, _>("id")? as u32),
            name: row.try_get("name")?,
            username: row.try_get("username")?,
            password_hash: row.try_get("password_hash")?,
            is_admin: row.try_get::<i64, _>("is_admin")? != 0,
            must_change_password: row.try_get::<i64, _>("must_change_password")? != 0,
        })
    }
}

fn user_from_row(row: &SqliteRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: UserId(row.try_get::<i64, _>("id")? as u32),
        name: row.try_get("name")?,
        username: row.try_get("username")?,
        is_admin: row.try_get::<i64, _>("is_admin")? != 0,
        must_change_password: row.try_get::<i64, _>("must_change_password")? != 0,
    })
}

pub async fn find_by_username(username: &str) -> Result<Option<StoredUser>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, username, password_hash, is_admin, must_change_password \
         FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool())
    .await?
    .as_ref()
    .map(StoredUser::from_row)
    .transpose()
}

pub async fn find_by_id(id: UserId) -> Result<Option<StoredUser>, sqlx::Error> {
    sqlx::query(
        "SELECT id, name, username, password_hash, is_admin, must_change_password \
         FROM users WHERE id = ?",
    )
    .bind(id.0 as i64)
    .fetch_optional(pool())
    .await?
    .as_ref()
    .map(StoredUser::from_row)
    .transpose()
}

pub async fn list_users() -> Result<Vec<User>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, username, is_admin, must_change_password FROM users ORDER BY name",
    )
    .fetch_all(pool())
    .await?;

    rows.iter().map(user_from_row).collect()
}

pub async fn insert_user(
    name: &str,
    username: &str,
    password_hash: &str,
    is_admin: bool,
    must_change_password: bool,
) -> Result<UserId, sqlx::Error> {
    let now = current_timestamp();

    let id = sqlx::query(
        "INSERT INTO users (name, username, password_hash, is_admin, must_change_password, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(name)
    .bind(username)
    .bind(password_hash)
    .bind(is_admin as i64)
    .bind(must_change_password as i64)
    .bind(now)
    .execute(pool())
    .await?
    .last_insert_rowid();

    Ok(UserId(id as u32))
}

pub async fn set_password(
    id: UserId,
    password_hash: &str,
    must_change_password: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = ? WHERE id = ?")
        .bind(password_hash)
        .bind(must_change_password as i64)
        .bind(id.0 as i64)
        .execute(pool())
        .await?;

    Ok(())
}

pub async fn delete_user(id: UserId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id.0 as i64)
        .execute(pool())
        .await?;

    Ok(())
}

pub async fn admin_count() -> Result<i64, sqlx::Error> {
    sqlx::query("SELECT COUNT(*) AS count FROM users WHERE is_admin = 1")
        .fetch_one(pool())
        .await?
        .try_get("count")
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
