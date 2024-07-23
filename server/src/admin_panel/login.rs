use crate::db::db;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::Request, middleware, response::Response};
use serde::Deserialize;
use sqlx::FromRow;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::Instant,
};

const SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

static SESSIONS: LazyLock<Mutex<HashMap<SessionId, SessionData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(u128);
impl SessionId {
    pub fn new() -> Self {
        Self(rand::random())
    }

    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

pub struct SessionData {
    last_activity: Instant,
    user_id: i64,
}

impl SessionData {
    pub fn new(user_id: i64) -> Self {
        Self {
            user_id,
            last_activity: Instant::now(),
        }
    }
}

#[derive(Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    password: String,
}

impl core::fmt::Debug for User {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct AuthState(Option<User>);
impl AuthState {
    pub fn is_authenticated(&self) -> bool {
        self.0.is_some()
    }

    pub fn user(&self) -> Option<&User> {
        self.0.as_ref()
    }
}

pub async fn auth(mut req: Request, next: middleware::Next) -> Response {
    let user_id = req
        .headers()
        .get_all("Cookie")
        .iter()
        .filter_map(|cookie| {
            let cookie_str = cookie.to_str().ok()?;
            cookie::Cookie::parse(cookie_str).ok()
        })
        .find_map(|cookie| {
            if cookie.name() == "session_id" {
                Some(SessionId(cookie.value().parse().ok()?))
            } else {
                None
            }
        })
        .and_then(|session_id| {
            let mut sessions = SESSIONS.lock().unwrap();
            let session = sessions.get_mut(&session_id)?;
            if session.last_activity.elapsed() > SESSION_TIMEOUT {
                None
            } else {
                session.last_activity = Instant::now();
                Some(session.user_id)
            }
        });

    let user = match user_id {
        Some(user_id) => {
            let db = db().await;

            sqlx::query_as!(
                User,
                "SELECT id AS 'id!', username, password FROM users WHERE id = ?",
                user_id
            )
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
        }
        None => None,
    };

    req.extensions_mut().insert(AuthState(user));

    next.run(req).await
}

#[derive(Deserialize)]
pub struct LoginData {
    username: String,
    password: String,
}

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
    InvalidUsername,
    InternalError,
}

impl core::fmt::Display for LoginError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // don't let the user know whether the username or password was wrong
            Self::InvalidCredentials => write!(f, "Invalid credentials"),
            Self::InvalidUsername => write!(f, "Invalid credentials"),
            Self::InternalError => write!(f, "Internal error"),
        }
    }
}

impl From<sqlx::Error> for LoginError {
    fn from(_: sqlx::Error) -> Self {
        Self::InternalError
    }
}
impl From<argon2::password_hash::Error> for LoginError {
    fn from(_: argon2::password_hash::Error) -> Self {
        Self::InternalError
    }
}

pub async fn login(data: LoginData) -> Result<SessionId, LoginError> {
    let db = db().await;
    let user = sqlx::query_as!(
        User,
        "SELECT id AS 'id!', username, password FROM users WHERE username = ?",
        data.username
    )
    .fetch_optional(db)
    .await?;

    let user = match user {
        Some(user) => user,
        None => {
            let total_users: i32 = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
                .fetch_one(db)
                .await?;

            if total_users == 0 {
                // Create a new admin user
                let salt = SaltString::generate(&mut OsRng);
                let argon2 = Argon2::default();
                let password_hash = argon2.hash_password(data.password.as_bytes(), &salt)?;
                let hash_str = password_hash.to_string();

                let user = sqlx::query_as!(
                    User,
                    "INSERT INTO users (username, password) VALUES ('admin', ?) RETURNING *",
                    hash_str
                )
                .fetch_one(db)
                .await?;
                user
            } else {
                Err(LoginError::InvalidUsername)?
            }
        }
    };

    let argon2 = Argon2::default();
    let password_hash = PasswordHash::new(&user.password).unwrap();
    let password_verifier = argon2.verify_password(data.password.as_bytes(), &password_hash);
    if password_verifier.is_ok() {
        let session_id = SessionId::new();
        SESSIONS
            .lock()
            .unwrap()
            .insert(session_id, SessionData::new(user.id));

        Ok(session_id)
    } else {
        Err(LoginError::InvalidCredentials)
    }
}
