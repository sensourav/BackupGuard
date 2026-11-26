use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use sha2::{Digest, Sha256};

use crate::db::Database;

pub const REDIRECT_URI: &str = "http://127.0.0.1:43821/callback";
const REDIRECT_PORT: u16 = 43821;
const AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

/// File reads stay on Files.Read. The other scopes only identify the account and refresh the session.
pub const SCOPES: &str = "openid profile offline_access https://graph.microsoft.com/User.Read https://graph.microsoft.com/Files.Read";

const EXAMPLE_CLIENT_ID: &str = "00000000-0000-0000-0000-000000000000";
static CONFIG_PATHS: OnceLock<Vec<std::path::PathBuf>> = OnceLock::new();

pub fn install_config_paths(paths: Vec<std::path::PathBuf>) {
    let _ = CONFIG_PATHS.set(paths);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkState {
    Connected,
    Offline,
    Expired,
}

#[derive(Debug)]
pub enum AuthError {
    SignedOut,
    Expired,
    Message(String),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveStatus {
    pub state: String,
    pub account: Option<String>,
}

pub fn status(db: &Database) -> Result<OneDriveStatus, String> {
    match link_state(db) {
        Ok(LinkState::Connected) => {
            let account = db
                .oauth_session()?
                .map(|session| session.account)
                .filter(|account| !account.is_empty());
            Ok(OneDriveStatus {
                state: "CONNECTED".to_string(),
                account,
            })
        }
        Ok(LinkState::Offline) => Ok(OneDriveStatus {
            state: "OFFLINE".to_string(),
            account: None,
        }),
        Ok(LinkState::Expired) => {
            let account = db.oauth_session()?.and_then(|session| {
                if session.account.is_empty() {
                    None
                } else {
                    Some(session.account)
                }
            });
            Ok(OneDriveStatus {
                state: "EXPIRED".to_string(),
                account,
            })
        }
        Err(err) => Err(err),
    }
}

pub fn link_state(db: &Database) -> Result<LinkState, String> {
    match ensure_access_token(db) {
        Ok(_) => Ok(LinkState::Connected),
        Err(AuthError::SignedOut) => Ok(LinkState::Offline),
        Err(AuthError::Expired) => Ok(LinkState::Expired),
        Err(AuthError::Message(_)) => match db.oauth_session() {
            Ok(Some(_)) => Ok(LinkState::Expired),
            Ok(None) => Ok(LinkState::Offline),
            Err(err) => Err(err),
        },
    }
}

pub fn ensure_access_token(db: &Database) -> Result<String, AuthError> {
    let Some(session) = db.oauth_session().map_err(AuthError::Message)? else {
        return Err(AuthError::SignedOut);
    };
    if access_is_current(&session.expires_at) {
        return unprotect_text(&session.access_token);
    }
    let refresh = unprotect_text(&session.refresh_token)?;
    let account = session.account.clone();
    let client_id = load_client_id_from_env_paths().map_err(AuthError::Message)?;
    match refresh_tokens(&client_id, &refresh) {
        Ok(tokens) => {
            let stored = TokenSet {
                account,
                ..tokens
            };
            store_tokens(db, &stored).map_err(AuthError::Message)?;
            Ok(stored.access_token)
        }
        Err(AuthError::Expired) => Err(AuthError::Expired),
        Err(other) => Err(other),
    }
}

pub fn connect(db: &Database, config_paths: &[std::path::PathBuf]) -> Result<OneDriveStatus, String> {
    let client_id = load_client_id(config_paths)?;
    let verifier = code_verifier()?;
    let challenge = code_challenge(&verifier);
    let state = code_verifier()?;
    let url = authorize_url(&client_id, &challenge, &state);
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|err| {
        format!(
            "could not listen on {REDIRECT_URI} ({err}). Register that redirect on the Azure app and close anything else using port {REDIRECT_PORT}."
        )
    })?;
    listener
        .set_nonblocking(false)
        .map_err(|err| err.to_string())?;
    open_browser(&url)?;
    let code = wait_for_code(&listener, &state)?;
    let tokens = exchange_code(&client_id, &code, &verifier)?;
    let account = fetch_account(&tokens.access_token).unwrap_or_default();
    let stored = TokenSet {
        account,
        ..tokens
    };
    store_tokens(db, &stored)?;
    status(db)
}

pub fn disconnect(db: &Database) -> Result<OneDriveStatus, String> {
    db.clear_oauth_session()?;
    status(db)
}

pub fn config_paths(app_data: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = vec![app_data.join("onedrive.client.local.json")];
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("onedrive.client.local.json"));
        if let Some(parent) = cwd.parent() {
            paths.push(parent.join("onedrive.client.local.json"));
        }
    }
    paths
}

fn load_client_id_from_env_paths() -> Result<String, String> {
    if let Some(paths) = CONFIG_PATHS.get() {
        return load_client_id(paths);
    }
    let mut paths = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(cwd.join("onedrive.client.local.json"));
        if let Some(parent) = cwd.parent() {
            paths.push(parent.join("onedrive.client.local.json"));
        }
    }
    if let Some(config) = std::env::var_os("GUARDSENPAI_ONEDRIVE_CONFIG") {
        paths.insert(0, std::path::PathBuf::from(config));
    }
    load_client_id(&paths)
}

pub fn load_client_id(paths: &[std::path::PathBuf]) -> Result<String, String> {
    for path in paths {
        if path.is_file() {
            let text = std::fs::read_to_string(path)
                .map_err(|err| format!("{}: {err}", path.display()))?;
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))?;
            let id = value
                .get("clientId")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            return validate_client_id(id);
        }
    }
    Err(format!(
        "Create onedrive.client.local.json with a clientId. Register {REDIRECT_URI} as a public-client redirect on the Azure app."
    ))
}

fn validate_client_id(id: &str) -> Result<String, String> {
    if id.is_empty() || id == EXAMPLE_CLIENT_ID {
        return Err(
            "onedrive.client.local.json needs the Azure application (client) id".to_string(),
        );
    }
    if id.len() < 8
        || !id
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || ch == '-')
    {
        return Err("clientId must be the Azure application id".to_string());
    }
    Ok(id.to_string())
}

struct TokenSet {
    access_token: String,
    refresh_token: String,
    expires_at: String,
    account: String,
}

fn store_tokens(db: &Database, tokens: &TokenSet) -> Result<(), String> {
    let access = protect(tokens.access_token.as_bytes())?;
    let refresh = protect(tokens.refresh_token.as_bytes())?;
    db.save_oauth_session(&tokens.account, &tokens.expires_at, &access, &refresh)
}

fn access_is_current(expires_at: &str) -> bool {
    let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) else {
        return false;
    };
    expires > Utc::now() + ChronoDuration::seconds(60)
}

fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    format!(
        "{AUTH_URL}?client_id={}&response_type=code&redirect_uri={}&response_mode=query&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        form_encode(client_id),
        form_encode(REDIRECT_URI),
        form_encode(SCOPES),
        form_encode(challenge),
        form_encode(state),
    )
}

fn exchange_code(client_id: &str, code: &str, verifier: &str) -> Result<TokenSet, String> {
    let body = form_pairs(&[
        ("client_id", client_id),
        ("scope", SCOPES),
        ("code", code),
        ("redirect_uri", REDIRECT_URI),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier),
    ]);
    post_token(&body)
}

fn refresh_tokens(client_id: &str, refresh_token: &str) -> Result<TokenSet, AuthError> {
    let body = form_pairs(&[
        ("client_id", client_id),
        ("scope", SCOPES),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ]);
    post_token(&body).map_err(|message| {
        if message.contains("invalid_grant") || message.contains("401") {
            AuthError::Expired
        } else {
            AuthError::Message(message)
        }
    })
}

fn post_token(body: &str) -> Result<TokenSet, String> {
    let response = agent()
        .post(TOKEN_URL)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(body)
        .map_err(|err| format!("OneDrive token request failed: {err}"))?;
    let text = response
        .into_string()
        .map_err(|err| format!("OneDrive token response could not be read: {err}"))?;
    let payload: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("OneDrive token response was not JSON: {err}"))?;
    token_set_from_payload(&payload, "")
}

fn token_set_from_payload(payload: &serde_json::Value, account: &str) -> Result<TokenSet, String> {
    if let Some(error) = payload.get("error").and_then(|item| item.as_str()) {
        let description = payload
            .get("error_description")
            .and_then(|item| item.as_str())
            .unwrap_or(error);
        return Err(format!("{error}: {description}"));
    }
    let access_token = payload
        .get("access_token")
        .and_then(|item| item.as_str())
        .ok_or_else(|| "OneDrive did not return an access token".to_string())?;
    let refresh_token = payload
        .get("refresh_token")
        .and_then(|item| item.as_str())
        .ok_or_else(|| "OneDrive did not return a refresh token".to_string())?;
    let expires_in = payload
        .get("expires_in")
        .and_then(|item| item.as_i64())
        .unwrap_or(3600);
    let expires_at = (Utc::now() + ChronoDuration::seconds(expires_in.max(0))).to_rfc3339();
    Ok(TokenSet {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        expires_at,
        account: account.to_string(),
    })
}

fn fetch_account(access_token: &str) -> Result<String, String> {
    let url = "https://graph.microsoft.com/v1.0/me?$select=userPrincipalName,mail";
    let response = agent()
        .get(url)
        .set("Authorization", &format!("Bearer {access_token}"))
        .call()
        .map_err(|err| format!("could not read the OneDrive account: {err}"))?;
    let text = response
        .into_string()
        .map_err(|err| format!("OneDrive account response could not be read: {err}"))?;
    let payload: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("OneDrive account response was not JSON: {err}"))?;
    let mail = payload
        .get("mail")
        .and_then(|item| item.as_str())
        .filter(|value| !value.is_empty());
    let upn = payload
        .get("userPrincipalName")
        .and_then(|item| item.as_str())
        .filter(|value| !value.is_empty());
    mail.or(upn)
        .map(str::to_string)
        .ok_or_else(|| "OneDrive account has no address".to_string())
}

fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<String, String> {
    listener
        .set_nonblocking(true)
        .map_err(|err| err.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("OneDrive sign-in timed out. Try Connect again.".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(format!("OneDrive sign-in was interrupted: {err}")),
        }
    };
    stream
        .set_nonblocking(false)
        .map_err(|err| err.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| err.to_string())?;
    let mut buffer = [0u8; 8192];
    let mut received = Vec::new();
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|err| format!("OneDrive sign-in was interrupted: {err}"))?;
        if read == 0 {
            break;
        }
        received.extend_from_slice(&buffer[..read]);
        if received.windows(4).any(|window| window == b"\r\n\r\n") || received.len() > 16_384 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&received);
    let page = "<!doctype html><html><body style=\"font-family:sans-serif;background:#020617;color:#e4e4e7\"><p>Guard Senpai signed in. You can close this window.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let request_line = text.lines().next().unwrap_or("");
    let query = request_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split_once('?').map(|(_, query)| query))
        .unwrap_or("");
    if let Some(error) = query_param(query, "error") {
        let description = query_param(query, "error_description").unwrap_or(error);
        return Err(format!("OneDrive sign-in was not completed: {description}"));
    }
    let state = query_param(query, "state").unwrap_or_default();
    if state != expected_state {
        return Err("OneDrive sign-in response did not match this Guard Senpai session".to_string());
    }
    query_param(query, "code").filter(|code| !code.is_empty()).ok_or_else(|| {
        "OneDrive did not return a sign-in code".to_string()
    })
}

fn open_browser(url: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let operation: Vec<u16> = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let file: Vec<u16> = std::ffi::OsStr::new(url)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        return Err("could not open the browser for OneDrive login".to_string());
    }
    Ok(())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(45))
        .build()
}

pub fn code_verifier() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    random_bytes(&mut bytes)?;
    Ok(base64url(&bytes))
}

pub fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64url(&digest)
}

fn random_bytes(buffer: &mut [u8]) -> Result<(), String> {
    use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};

    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        return Err("could not generate the OneDrive login verifier".to_string());
    }
    Ok(())
}

fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("could not encrypt the OneDrive token".to_string());
    }
    let encrypted = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(output.pbData as *mut _);
    }
    Ok(encrypted)
}

fn unprotect_text(blob: &[u8]) -> Result<String, AuthError> {
    let plain = unprotect(blob).map_err(AuthError::Message)?;
    String::from_utf8(plain).map_err(|_| AuthError::Message("stored OneDrive token was unreadable".to_string()))
}

fn unprotect(blob: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    let input = CRYPT_INTEGER_BLOB {
        cbData: blob.len() as u32,
        pbData: blob.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err("could not decrypt the OneDrive token".to_string());
    }
    let plain = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(output.pbData as *mut _);
    }
    Ok(plain)
}

pub fn base64url(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let chunk = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        out.push(TABLE[((chunk >> 18) & 63) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 63) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 63) as usize] as char);
        out.push(TABLE[(chunk & 63) as usize] as char);
        index += 3;
    }
    let rest = data.len() - index;
    if rest == 1 {
        let chunk = (data[index] as u32) << 16;
        out.push(TABLE[((chunk >> 18) & 63) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 63) as usize] as char);
    } else if rest == 2 {
        let chunk = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8);
        out.push(TABLE[((chunk >> 18) & 63) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 63) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 63) as usize] as char);
    }
    out
}

fn form_pairs(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if percent_decode(name) == key {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(decoded) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(decoded);
                index += 3;
                continue;
            }
        }
        if bytes[index] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[index]);
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_scope_is_read_only() {
        assert!(SCOPES.contains("https://graph.microsoft.com/Files.Read"));
        assert!(!SCOPES.contains("ReadWrite"));
        assert!(!SCOPES.contains("Files.Read.All"));
        assert!(authorize_url("client", "challenge", "state").contains("code_challenge_method=S256"));
    }

    #[test]
    fn pkce_challenge_is_base64url_sha256() {
        let challenge = code_challenge("guardsenpai-verifier");
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
    }

    #[test]
    fn token_blobs_round_trip_without_plaintext() {
        let secret = b"access-token-sample";
        let blob = protect(secret).unwrap();
        assert_ne!(blob.as_slice(), secret.as_slice());
        assert!(!String::from_utf8_lossy(&blob).contains("access-token-sample"));
        assert_eq!(unprotect(&blob).unwrap(), secret);
    }

    #[test]
    fn example_client_id_is_rejected() {
        let error = validate_client_id(EXAMPLE_CLIENT_ID).unwrap_err();
        assert!(error.contains("client"));
    }
}
