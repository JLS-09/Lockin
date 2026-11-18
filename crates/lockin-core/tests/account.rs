//! Making an account, logging in, refreshing and logging out, against a real
//! Supabase Auth (GoTrue) on this computer with Lockin's cloud schema and its
//! invite hook. `tools/test-cloud-auth.sh` starts one and runs these tests; a
//! plain `cargo test` skips them (no LOCKIN_TEST_AUTH_URL).

use lockin_core::account::{self, AccountError, Cloud, Kept};

/// The local Auth service and its database, or None to skip.
fn local() -> Option<(Cloud, String)> {
    let auth_url = std::env::var("LOCKIN_TEST_AUTH_URL").ok()?;
    let database = std::env::var("LOCKIN_TEST_DATABASE_URL").ok()?;
    Some((Cloud::new(&auth_url, "test-key"), database))
}

macro_rules! local_or_skip {
    () => {
        match local() {
            Some(found) => found,
            None => {
                eprintln!("skipped: run tools/test-cloud-auth.sh for the account tests");
                return;
            }
        }
    };
}

/// Run SQL on the local database (as the project owner would in the SQL Editor).
fn sql(database: &str, query: &str) -> String {
    let output = std::process::Command::new("psql")
        .args([
            database,
            "-X",
            "-q",
            "-A",
            "-t",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            query,
        ])
        .output()
        .expect("psql runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn new_invite(database: &str) -> String {
    sql(
        database,
        "insert into public.invite_code (note) values ('test') returning code",
    )
}

/// An email nobody used yet, so the tests can run again on the same database.
fn new_email() -> String {
    format!("{}@example.test", lockin_core::new_id())
}

fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

#[tokio::test]
async fn an_invite_makes_an_account() {
    let (cloud, database) = local_or_skip!();
    let email = new_email();

    // No code, or one that doesn't exist: no account.
    assert_eq!(
        account::sign_up(&cloud, &email, "secret123", "").await,
        Err(AccountError::InvalidInvite)
    );
    assert_eq!(
        account::sign_up(&cloud, &email, "secret123", "NOPE-NOPE").await,
        Err(AccountError::InvalidInvite)
    );

    // A short password is refused before the code is used up.
    let code = new_invite(&database);
    assert_eq!(
        account::sign_up(&cloud, &email, "123", &code).await,
        Err(AccountError::WeakPassword)
    );

    // As a friend would type it: lower case, with a dash.
    let typed = format!("{}-{}", &code[..4], &code[4..]).to_lowercase();
    let session = account::sign_up(&cloud, &email, "secret123", &typed)
        .await
        .unwrap();
    assert_eq!(session.email, email);
    assert!(session.expires_at > now_unix());
    assert_eq!(
        sql(
            &database,
            &format!("select used_by from public.invite_code where code = '{code}'")
        ),
        session.user_id,
        "the code remembers who used it"
    );

    // The code works once, and the email is taken now.
    assert_eq!(
        account::sign_up(&cloud, &new_email(), "secret123", &code).await,
        Err(AccountError::InvalidInvite)
    );
    let another = new_invite(&database);
    assert_eq!(
        account::sign_up(&cloud, &email, "secret123", &another).await,
        Err(AccountError::EmailTaken)
    );
}

#[tokio::test]
async fn logging_in_gives_the_same_feed_key() {
    let (cloud, database) = local_or_skip!();
    let email = new_email();
    let signed_up = account::sign_up(&cloud, &email, "secret123", &new_invite(&database))
        .await
        .unwrap();

    assert_eq!(
        account::log_in(&cloud, &email, "wrong-password").await,
        Err(AccountError::WrongLogin)
    );
    let logged_in = account::log_in(&cloud, &format!(" {email} "), "secret123")
        .await
        .unwrap();
    assert_eq!(logged_in.user_id, signed_up.user_id);
    assert_eq!(
        logged_in.feed_key, signed_up.feed_key,
        "another computer can read the feed links"
    );
}

#[tokio::test]
async fn the_daemon_keeps_the_login_fresh() {
    let (cloud, database) = local_or_skip!();
    let folder = std::env::temp_dir().join(format!("lockin-test-{}", lockin_core::new_id()));
    let file = folder.join("account.json");
    let email = new_email();
    let session = account::sign_up(&cloud, &email, "secret123", &new_invite(&database))
        .await
        .unwrap();
    account::save(&file, &session).unwrap();

    // Plenty of time left: nothing to do.
    assert_eq!(
        account::keep_fresh(&cloud, &file, now_unix())
            .await
            .unwrap(),
        Kept::Nothing
    );
    assert_eq!(account::load(&file).unwrap(), Some(session.clone()));

    // Close to the end: a new refresh token, the same feed key.
    let later = session.expires_at - 60;
    assert_eq!(
        account::keep_fresh(&cloud, &file, later).await.unwrap(),
        Kept::Refreshed
    );
    let fresh = account::load(&file).unwrap().unwrap();
    assert_ne!(fresh.refresh_token, session.refresh_token);
    assert_eq!(fresh.feed_key, session.feed_key);

    // Logged out (on the server): the next refresh ends the login here too.
    account::log_out(&cloud, &fresh).await.unwrap();
    assert_eq!(
        account::keep_fresh(&cloud, &file, fresh.expires_at - 60)
            .await
            .unwrap(),
        Kept::LoggedOut
    );
    assert_eq!(account::load(&file).unwrap(), None);

    // Logging out twice is fine.
    account::log_out(&cloud, &fresh).await.unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

/// The way supabase/README.md resets a forgotten password by hand.
#[tokio::test]
async fn a_password_reset_by_hand_works() {
    let (cloud, database) = local_or_skip!();
    let email = new_email();
    let before = account::sign_up(&cloud, &email, "secret123", &new_invite(&database))
        .await
        .unwrap();

    sql(
        &database,
        &format!(
            "update auth.users
             set encrypted_password = extensions.crypt('new-secret-456', extensions.gen_salt('bf'))
             where email = '{email}'"
        ),
    );

    assert_eq!(
        account::log_in(&cloud, &email, "secret123").await,
        Err(AccountError::WrongLogin)
    );
    let after = account::log_in(&cloud, &email, "new-secret-456")
        .await
        .unwrap();
    assert_eq!(after.user_id, before.user_id);
    // A new password, a new feed key: feed links are pasted again (as planned).
    assert_ne!(after.feed_key, before.feed_key);
}
