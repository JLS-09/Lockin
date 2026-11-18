//! Your Lockin account: making one (with an invite code), logging in and out,
//! and keeping the login on this computer.
//!
//! Everyone who uses Lockin shares one Supabase project (see supabase/README.md).
//! Accounts live in its Auth service, which Lockin talks to over plain HTTPS:
//! - sign up:  POST {auth}/signup                       (email, password, invite code)
//! - log in:   POST {auth}/token?grant_type=password
//! - refresh:  POST {auth}/token?grant_type=refresh_token
//! - log out:  POST {auth}/logout
//!
//! A login is a `Session`: a short-lived access token (an hour) and a refresh
//! token to get a new one. It's kept in `~/.config/lockin/account.json`, readable
//! only by you. The daemon refreshes it before it runs out (`keep_fresh`); the app
//! only logs in and out, so the two never use the same refresh token twice.
//!
//! The session also holds the key that encrypts feed links before they're
//! synced. It's worked out from your password when you log in (`feed_key`), so
//! every computer you log in on gets the same key, and the password itself is
//! never stored.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The shared Supabase project. Both values are public by design: they only say
/// which project to talk to. Row-level security keeps everyone's rows apart.
pub const PROJECT_URL: &str = "https://czencbvwskzufcqeiwvj.supabase.co";
pub const PUBLISHABLE_KEY: &str = "sb_publishable_XUZDW0mPpvJDW3JG4useHg_yhHPRdaQ";

/// Refresh the login once it has less than this left (it lasts an hour).
pub const REFRESH_BEFORE_SECONDS: i64 = 10 * 60;
/// Give up on a request to the account server after this long.
const TIMEOUT_SECONDS: u64 = 20;

/// The message the invite hook answers with (supabase/migrations, `lockin_check_invite`).
const INVITE_REFUSED: &str = "lockin: invalid invite code";

/// Where the account server is.
#[derive(Debug, Clone)]
pub struct Cloud {
    /// The Auth service, e.g. https://….supabase.co/auth/v1
    pub auth_url: String,
    pub key: String,
}

impl Cloud {
    /// Lockin's project. For development and tests, `LOCKIN_AUTH_URL` points it
    /// at a local Supabase Auth instead (tools/test-cloud-auth.sh).
    pub fn lockin() -> Cloud {
        let auth_url =
            std::env::var("LOCKIN_AUTH_URL").unwrap_or_else(|_| format!("{PROJECT_URL}/auth/v1"));
        Cloud::new(&auth_url, PUBLISHABLE_KEY)
    }

    pub fn new(auth_url: &str, key: &str) -> Cloud {
        Cloud {
            auth_url: auth_url.trim_end_matches('/').to_string(),
            key: key.to_string(),
        }
    }
}

/// A login, as kept in account.json. Never sent to the frontend: it holds the tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    /// When the access token stops working, in Unix seconds.
    pub expires_at: i64,
    /// The key that encrypts feed links (hex, 32 bytes), from the password.
    pub feed_key: String,
}

/// What the app may know about the login.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub email: String,
}

/// Why making an account, logging in or refreshing didn't work.
#[derive(Debug, Clone, PartialEq)]
pub enum AccountError {
    /// The invite code is missing, unknown, used or expired.
    InvalidInvite,
    /// Wrong email or password.
    WrongLogin,
    /// There's already an account with this email.
    EmailTaken,
    /// The password is too short (at least 6 characters).
    WeakPassword,
    /// Not an email address.
    InvalidEmail,
    /// The login ran out or was ended elsewhere: log in again.
    LoggedOut,
    /// The account server couldn't be reached.
    Offline,
    /// Anything else, with the server's message.
    Server(String),
}

impl AccountError {
    /// The translation key the app shows (en.json / nl.json).
    pub fn key(&self) -> &'static str {
        match self {
            AccountError::InvalidInvite => "account.error.invite",
            AccountError::WrongLogin => "account.error.wrongLogin",
            AccountError::EmailTaken => "account.error.emailTaken",
            AccountError::WeakPassword => "account.error.weakPassword",
            AccountError::InvalidEmail => "account.error.invalidEmail",
            AccountError::LoggedOut => "account.error.loggedOut",
            AccountError::Offline => "account.error.offline",
            AccountError::Server(_) => "account.error.server",
        }
    }
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::InvalidInvite => write!(f, "the invite code isn't valid (or was used)"),
            AccountError::WrongLogin => write!(f, "wrong email or password"),
            AccountError::EmailTaken => write!(f, "there's already an account with this email"),
            AccountError::WeakPassword => write!(f, "the password is too short"),
            AccountError::InvalidEmail => write!(f, "that's not an email address"),
            AccountError::LoggedOut => write!(f, "the login ran out: log in again"),
            AccountError::Offline => write!(f, "the account server can't be reached"),
            AccountError::Server(message) => write!(f, "the account server said: {message}"),
        }
    }
}

impl std::error::Error for AccountError {}

/// An invite code as typed ("k7qm-2xwd ") → as stored ("K7QM2XWD").
pub fn clean_invite_code(typed: &str) -> String {
    typed
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The code in an invite link (`lockin://invite/K7QM-2XWD`), if `text` is one.
pub fn invite_from_link(text: &str) -> Option<String> {
    let rest = text.strip_prefix("lockin://invite/")?;
    let code = clean_invite_code(rest);
    (!code.is_empty()).then_some(code)
}

/// Make an account and log in. The invite code goes along as user metadata;
/// the server's invite hook checks it and marks it used.
pub async fn sign_up(
    cloud: &Cloud,
    email: &str,
    password: &str,
    invite_code: &str,
) -> Result<Session, AccountError> {
    let code = clean_invite_code(invite_code);
    if code.is_empty() {
        return Err(AccountError::InvalidInvite);
    }
    let body = json!({
        "email": email.trim(),
        "password": password,
        "data": { "invite_code": code },
    });
    let answer = post(cloud, "/signup", None, &body).await?;
    session_from(&answer, password)
}

/// Log in with email and password.
pub async fn log_in(cloud: &Cloud, email: &str, password: &str) -> Result<Session, AccountError> {
    let body = json!({ "email": email.trim(), "password": password });
    let answer = post(cloud, "/token?grant_type=password", None, &body).await?;
    session_from(&answer, password)
}

/// A new access token for a session. The refresh token changes too (each one
/// works once), so save what this returns. The feed key stays.
pub async fn refresh(cloud: &Cloud, session: &Session) -> Result<Session, AccountError> {
    let body = json!({ "refresh_token": session.refresh_token });
    let answer = post(cloud, "/token?grant_type=refresh_token", None, &body)
        .await
        .map_err(|error| match error {
            // A refused refresh token means the login is over. Anything else
            // (offline, a server hiccup) is tried again later.
            AccountError::WrongLogin => AccountError::LoggedOut,
            other => other,
        })?;
    let mut fresh = parse_session(&answer)?;
    fresh.feed_key = session.feed_key.clone();
    Ok(fresh)
}

/// End the login on the server too. A token that already ran out is fine:
/// the login is over either way.
pub async fn log_out(cloud: &Cloud, session: &Session) -> Result<(), AccountError> {
    match post(cloud, "/logout", Some(&session.access_token), &json!({})).await {
        Ok(_) | Err(AccountError::WrongLogin | AccountError::LoggedOut) => Ok(()),
        Err(error) => Err(error),
    }
}

/// True when the session should be refreshed now.
pub fn refresh_due(session: &Session, now_unix: i64) -> bool {
    session.expires_at - now_unix < REFRESH_BEFORE_SECONDS
}

/// What `keep_fresh` did.
#[derive(Debug, PartialEq)]
pub enum Kept {
    /// Not logged in, or the login has time left.
    Nothing,
    Refreshed,
    /// The server ended the login; account.json is gone.
    LoggedOut,
}

/// Refresh the saved login when it's about to run out (the daemon calls this
/// every few seconds). If the app logged out or in again meanwhile, the new
/// file is kept: it's only replaced when it still holds the session refreshed.
pub async fn keep_fresh(cloud: &Cloud, file: &Path, now_unix: i64) -> Result<Kept> {
    let Some(session) = load(file)? else {
        return Ok(Kept::Nothing);
    };
    if !refresh_due(&session, now_unix) {
        return Ok(Kept::Nothing);
    }
    let result = refresh(cloud, &session).await;
    // Read again: the app may have changed the file while we waited.
    if load(file)?.as_ref() != Some(&session) {
        return Ok(Kept::Nothing);
    }
    match result {
        Ok(fresh) => {
            save(file, &fresh)?;
            Ok(Kept::Refreshed)
        }
        Err(AccountError::LoggedOut) => {
            remove(file)?;
            Ok(Kept::LoggedOut)
        }
        Err(error) => Err(error.into()),
    }
}

// ---------- account.json ----------

/// The saved login, or None when not logged in.
pub fn load(file: &Path) -> Result<Option<Session>> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", file.display())),
    };
    let session = serde_json::from_str(&text)
        .with_context(|| format!("{} isn't a Lockin login", file.display()))?;
    Ok(Some(session))
}

/// Save the login, readable only by you. Written to a temporary file first and
/// then renamed, so the daemon never reads half a file.
pub fn save(file: &Path, session: &Session) -> Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let temporary = file.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(session)?;
    write_private(&temporary, text.as_bytes())
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, file).with_context(|| format!("saving {}", file.display()))?;
    Ok(())
}

/// Forget the login on this computer.
pub fn remove(file: &Path) -> Result<()> {
    match std::fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", file.display())),
    }
}

#[cfg(unix)]
fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(file)?;
    out.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(file, bytes)
}

// ---------- The feed key ----------

/// The key that encrypts feed links: Argon2id of the password, salted with the
/// account's id, so it's the same on every computer and different per account.
/// A new password gives a new key (feed links then have to be pasted again).
pub fn feed_key(password: &str, user_id: &str) -> String {
    let salt = format!("lockin-feed-key:{user_id}");
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt.as_bytes(), &mut key)
        .expect("32 bytes and a salt of 16+ bytes are always accepted");
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

// ---------- Talking to the server ----------

/// POST JSON to the Auth service and return the JSON answer, or the error it means.
async fn post(
    cloud: &Cloud,
    path: &str,
    bearer: Option<&str>,
    body: &Value,
) -> Result<Value, AccountError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECONDS))
        .build()
        .map_err(|error| AccountError::Server(error.to_string()))?;
    let mut request = client
        .post(format!("{}{path}", cloud.auth_url))
        .header("apikey", &cloud.key)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|_| AccountError::Offline)?;
    let status = response.status().as_u16();
    let text = response.text().await.map_err(|_| AccountError::Offline)?;
    if (200..300).contains(&status) {
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        return serde_json::from_str(&text).map_err(|_| AccountError::Server(text));
    }
    Err(parse_error(status, &text))
}

/// What an error answer from Supabase Auth means. It looks like
/// `{"code":422,"error_code":"user_already_exists","msg":"User already registered"}`
/// (older servers: `{"error":"invalid_grant","error_description":"…"}`).
pub fn parse_error(status: u16, body: &str) -> AccountError {
    let answer: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let text = |key: &str| answer.get(key).and_then(Value::as_str).unwrap_or("");
    let code = text("error_code");
    let message = [
        text("msg"),
        text("message"),
        text("error_description"),
        text("error"),
    ]
    .into_iter()
    .find(|m| !m.is_empty())
    .unwrap_or(body)
    .to_string();

    if message.contains(INVITE_REFUSED) {
        return AccountError::InvalidInvite;
    }
    match code {
        "invalid_credentials" => AccountError::WrongLogin,
        "user_already_exists" | "email_exists" => AccountError::EmailTaken,
        "weak_password" => AccountError::WeakPassword,
        "email_address_invalid" | "validation_failed" => AccountError::InvalidEmail,
        "refresh_token_not_found"
        | "refresh_token_already_used"
        | "session_not_found"
        | "session_expired"
        | "bad_jwt"
        | "user_not_found" => AccountError::LoggedOut,
        _ if text("error") == "invalid_grant" => AccountError::WrongLogin,
        _ if status == 401 => AccountError::LoggedOut,
        _ => AccountError::Server(message),
    }
}

/// A login answer → a Session, with the feed key worked out from the password.
fn session_from(answer: &Value, password: &str) -> Result<Session, AccountError> {
    let mut session = parse_session(answer)?;
    session.feed_key = feed_key(password, &session.user_id);
    Ok(session)
}

/// The session in a login answer (without the feed key).
pub fn parse_session(answer: &Value) -> Result<Session, AccountError> {
    let text = |value: &Value| value.as_str().map(str::to_string);
    let user = &answer["user"];
    let found = (|| {
        Some(Session {
            user_id: text(&user["id"])?,
            email: text(&user["email"])?,
            access_token: text(&answer["access_token"])?,
            refresh_token: text(&answer["refresh_token"])?,
            expires_at: answer["expires_at"].as_i64()?,
            feed_key: String::new(),
        })
    })();
    // A new account without a login means "Confirm email" is on in the
    // project (supabase/README.md says to turn it off).
    found.ok_or_else(|| {
        AccountError::Server(
            "the server didn't log you in (is \"Confirm email\" turned off?)".to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_codes_are_cleaned() {
        assert_eq!(clean_invite_code(" k7qm-2xwd "), "K7QM2XWD");
        assert_eq!(
            invite_from_link("lockin://invite/K7QM-2XWD").as_deref(),
            Some("K7QM2XWD")
        );
        assert_eq!(invite_from_link("lockin://invite/"), None);
        assert_eq!(invite_from_link("--page"), None);
    }

    #[test]
    fn errors_from_the_server() {
        let error = |status, body| parse_error(status, body);
        assert_eq!(
            error(
                403,
                r#"{"code":403,"error_code":"unknown","msg":"lockin: invalid invite code"}"#
            ),
            AccountError::InvalidInvite
        );
        assert_eq!(
            error(
                400,
                r#"{"code":400,"error_code":"invalid_credentials","msg":"Invalid login credentials"}"#
            ),
            AccountError::WrongLogin
        );
        assert_eq!(
            error(
                422,
                r#"{"code":422,"error_code":"user_already_exists","msg":"User already registered"}"#
            ),
            AccountError::EmailTaken
        );
        assert_eq!(
            error(
                422,
                r#"{"code":422,"error_code":"weak_password","msg":"Password should be at least 6 characters."}"#
            ),
            AccountError::WeakPassword
        );
        assert_eq!(
            error(
                400,
                r#"{"code":400,"error_code":"refresh_token_not_found","msg":"Invalid Refresh Token: Refresh Token Not Found"}"#
            ),
            AccountError::LoggedOut
        );
        assert_eq!(
            error(
                400,
                r#"{"error":"invalid_grant","error_description":"Invalid login credentials"}"#
            ),
            AccountError::WrongLogin
        );
        assert_eq!(
            error(500, "Internal Server Error"),
            AccountError::Server("Internal Server Error".to_string())
        );
    }

    #[test]
    fn a_login_answer_becomes_a_session() {
        let answer = json!({
            "access_token": "a.b.c", "refresh_token": "r1", "expires_at": 1_800_000_000,
            "expires_in": 3600, "token_type": "bearer",
            "user": { "id": "2d5e92ab-d5cd-4254-9ffd-68aaebbb4720", "email": "a@example.test" }
        });
        let session = session_from(&answer, "secret123").unwrap();
        assert_eq!(session.email, "a@example.test");
        assert_eq!(session.expires_at, 1_800_000_000);
        assert_eq!(session.feed_key.len(), 64);

        // A signup that waits for email confirmation has no tokens.
        let waiting = json!({ "id": "x", "email": "a@example.test" });
        assert!(matches!(
            parse_session(&waiting),
            Err(AccountError::Server(_))
        ));
    }

    #[test]
    fn the_feed_key_follows_password_and_account() {
        let key = feed_key("secret123", "user-1");
        assert_eq!(
            key,
            feed_key("secret123", "user-1"),
            "the same on every computer"
        );
        assert_ne!(key, feed_key("secret124", "user-1"));
        assert_ne!(key, feed_key("secret123", "user-2"));
    }

    #[test]
    fn refresh_ten_minutes_before_the_end() {
        let mut session = sample();
        session.expires_at = 10_000;
        assert!(!refresh_due(&session, 10_000 - 3600));
        assert!(!refresh_due(&session, 10_000 - 601));
        assert!(refresh_due(&session, 10_000 - 599));
        assert!(refresh_due(&session, 20_000));
    }

    #[test]
    fn the_saved_login_is_private() {
        let folder = std::env::temp_dir().join(format!("lockin-account-{}", crate::new_id()));
        let file = folder.join("account.json");
        assert_eq!(load(&file).unwrap(), None);

        save(&file, &sample()).unwrap();
        assert_eq!(load(&file).unwrap(), Some(sample()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        remove(&file).unwrap();
        remove(&file).unwrap(); // already gone: fine
        assert_eq!(load(&file).unwrap(), None);
        std::fs::remove_dir_all(folder).unwrap();
    }

    fn sample() -> Session {
        Session {
            user_id: "user-1".to_string(),
            email: "a@example.test".to_string(),
            access_token: "a.b.c".to_string(),
            refresh_token: "r1".to_string(),
            expires_at: 1_800_000_000,
            feed_key: feed_key("secret123", "user-1"),
        }
    }
}
