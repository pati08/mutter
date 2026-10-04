//! Password hashing, sessions, and account operations.

use anyhow::{anyhow, Context};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::rngs::OsRng;
use rand::{distributions::Alphanumeric, Rng};
use tower_sessions::Session;

use crate::models::{User, UserId};
use crate::server::db;

const SESSION_KEY: &str = "user_id";
const MIN_PASSWORD_LEN: usize = 8;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(OsRng);

    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| anyhow!("could not hash password: {err}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

pub fn generate_temp_password() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

async fn session_user_id(session: &Session) -> Option<UserId> {
    session
        .get::<u32>(SESSION_KEY)
        .await
        .ok()
        .flatten()
        .map(UserId)
}

pub async fn current_user(session: &Session) -> anyhow::Result<Option<User>> {
    match session_user_id(session).await {
        Some(id) => Ok(db::find_by_id(id).await?.map(|stored| stored.into_user())),
        None => Ok(None),
    }
}

pub async fn require_user(session: &Session) -> anyhow::Result<User> {
    current_user(session)
        .await?
        .ok_or_else(|| anyhow!("you are not signed in"))
}

pub async fn require_admin(session: &Session) -> anyhow::Result<User> {
    let user = require_user(session).await?;
    if user.is_admin {
        Ok(user)
    } else {
        Err(anyhow!("administrator access is required"))
    }
}

pub async fn login(
    session: &Session,
    username: &str,
    password: &str,
) -> anyhow::Result<Option<User>> {
    let Some(stored) = db::find_by_username(username).await? else {
        return Ok(None);
    };

    if !verify_password(password, &stored.password_hash) {
        return Ok(None);
    }

    // Cycle the id to protect against session fixation.
    session
        .cycle_id()
        .await
        .context("could not cycle the session id")?;
    session
        .insert(SESSION_KEY, stored.id.0)
        .await
        .context("could not store the session")?;

    Ok(Some(stored.into_user()))
}

pub async fn logout(session: &Session) -> anyhow::Result<()> {
    session
        .flush()
        .await
        .context("could not clear the session")?;
    Ok(())
}

pub async fn change_password(session: &Session, current: &str, new: &str) -> anyhow::Result<User> {
    let user = require_user(session).await?;
    let stored = db::find_by_id(user.id)
        .await?
        .ok_or_else(|| anyhow!("account no longer exists"))?;

    if !verify_password(current, &stored.password_hash) {
        return Err(anyhow!("your current password is incorrect"));
    }
    if new.len() < MIN_PASSWORD_LEN {
        return Err(anyhow!(
            "the new password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    if new == current {
        return Err(anyhow!("the new password must be different"));
    }

    let hash = hash_password(new)?;
    db::set_password(user.id, &hash, false).await?;

    let updated = db::find_by_id(user.id)
        .await?
        .ok_or_else(|| anyhow!("account no longer exists"))?;
    Ok(updated.into_user())
}

pub async fn list_users(session: &Session) -> anyhow::Result<Vec<User>> {
    require_admin(session).await?;
    Ok(db::list_users().await?)
}

/// Stores the signed-in user's chosen accent color.
pub async fn set_theme_color(session: &Session, color: &str) -> anyhow::Result<User> {
    let user = require_user(session).await?;

    if !crate::theme::is_valid_hex(color) {
        return Err(anyhow!("that is not a valid color"));
    }

    db::set_theme_color(user.id, Some(color)).await?;

    let updated = db::find_by_id(user.id)
        .await?
        .ok_or_else(|| anyhow!("account no longer exists"))?;
    Ok(updated.into_user())
}

/// Creates an account and returns its one-time temporary password.
pub async fn create_user(session: &Session, name: &str, username: &str) -> anyhow::Result<String> {
    require_admin(session).await?;

    let name = name.trim();
    let username = username.trim();

    if name.is_empty() || username.is_empty() {
        return Err(anyhow!("name and username are required"));
    }
    if db::find_by_username(username).await?.is_some() {
        return Err(anyhow!("that username is already taken"));
    }

    let temp_password = generate_temp_password();
    let hash = hash_password(&temp_password)?;
    db::insert_user(name, username, &hash, false, true).await?;

    Ok(temp_password)
}

/// Issues a fresh temporary password and re-arms the forced change.
pub async fn reset_password(session: &Session, user_id: UserId) -> anyhow::Result<String> {
    require_admin(session).await?;

    if db::find_by_id(user_id).await?.is_none() {
        return Err(anyhow!("no such user"));
    }

    let temp_password = generate_temp_password();
    let hash = hash_password(&temp_password)?;
    db::set_password(user_id, &hash, true).await?;

    Ok(temp_password)
}

pub async fn delete_user(session: &Session, user_id: UserId) -> anyhow::Result<()> {
    let admin = require_admin(session).await?;

    if admin.id == user_id {
        return Err(anyhow!("you cannot delete your own account"));
    }

    let Some(target) = db::find_by_id(user_id).await? else {
        return Err(anyhow!("no such user"));
    };

    if target.is_admin && db::admin_count().await? <= 1 {
        return Err(anyhow!("the last administrator cannot be deleted"));
    }

    db::delete_user(user_id).await?;
    crate::server::queues::MESSAGE_QUEUES.remove_user(user_id);
    crate::server::connected_users::CONNECTED_USERS.disconnect_user(user_id);

    Ok(())
}
