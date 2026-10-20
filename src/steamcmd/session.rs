//! The Steam session: steamcmd's cached login (`config/config.vdf`), passed
//! as gzip + base64 through the environment only (spec sections 3 and 9).

use std::fmt;
use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

/// The Steam user name.
pub const USERNAME_VAR: &str = "PDX_WS_STEAM_USERNAME";
/// The Steam session: gzip + base64 of steamcmd's `config/config.vdf`.
pub const SESSION_VAR: &str = "PDX_WS_STEAM_SESSION";

/// A decoded session is far smaller; anything bigger is not a session.
const MAX_SESSION_BYTES: u64 = 4 * 1024 * 1024;

/// The user name and the decoded session. `Debug` never shows the session.
#[derive(Clone)]
pub struct Credentials {
    user: String,
    session: Vec<u8>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .field("session", &format_args!("<{} bytes>", self.session.len()))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("{USERNAME_VAR} is not set; set it to the Steam user name of the Steam session")]
    MissingUser,
    #[error(
        "{SESSION_VAR} is not set; create a Steam session with `pdx-ws session export` and \
         pass it in this environment variable"
    )]
    MissingSession,
    #[error(
        "{SESSION_VAR} is not a Steam session ({0}); create a new one with \
         `pdx-ws session export`"
    )]
    Invalid(&'static str),
}

impl Credentials {
    /// Reads [`USERNAME_VAR`] and [`SESSION_VAR`].
    pub fn from_env() -> Result<Credentials, SessionError> {
        let user = std::env::var(USERNAME_VAR).unwrap_or_default();
        let session = std::env::var(SESSION_VAR).unwrap_or_default();
        Credentials::new(&user, &session)
    }

    /// Decodes a gzip + base64 session for `user`.
    pub fn new(user: &str, encoded: &str) -> Result<Credentials, SessionError> {
        let user = user.trim();
        if user.is_empty() {
            return Err(SessionError::MissingUser);
        }
        let encoded: String = encoded.split_whitespace().collect();
        if encoded.is_empty() {
            return Err(SessionError::MissingSession);
        }
        let compressed = STANDARD
            .decode(encoded)
            .map_err(|_| SessionError::Invalid("not base64"))?;
        let mut session = Vec::new();
        GzDecoder::new(compressed.as_slice())
            .take(MAX_SESSION_BYTES + 1)
            .read_to_end(&mut session)
            .map_err(|_| SessionError::Invalid("not gzip"))?;
        if session.len() as u64 > MAX_SESSION_BYTES {
            return Err(SessionError::Invalid("too large"));
        }
        if session.is_empty() {
            return Err(SessionError::Invalid("empty"));
        }
        Ok(Credentials {
            user: user.to_owned(),
            session,
        })
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    /// The decoded `config.vdf`.
    pub fn session(&self) -> &[u8] {
        &self.session
    }
}

/// Encodes a `config.vdf` as a Steam session (gzip + base64).
pub fn encode(config_vdf: &[u8]) -> String {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(config_vdf)
        .expect("writing to a Vec cannot fail");
    let compressed = encoder.finish().expect("writing to a Vec cannot fail");
    STANDARD.encode(compressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_session() {
        let config = b"\"InstallConfigStore\"\n{\n}\n";
        let encoded = encode(config);
        let credentials = Credentials::new("modder", &format!(" {encoded}\n")).unwrap();
        assert_eq!(credentials.user(), "modder");
        assert_eq!(credentials.session(), config);
    }

    #[test]
    fn debug_hides_the_session() {
        let credentials = Credentials::new("modder", &encode(b"secret-token")).unwrap();
        let debug = format!("{credentials:?}");
        assert!(!debug.contains("secret"), "{debug}");
        assert!(debug.contains("<12 bytes>"), "{debug}");
    }

    #[test]
    fn rejects_missing_and_malformed_sessions() {
        assert_eq!(
            Credentials::new("", "x").unwrap_err(),
            SessionError::MissingUser
        );
        assert_eq!(
            Credentials::new("modder", "  ").unwrap_err(),
            SessionError::MissingSession
        );
        assert_eq!(
            Credentials::new("modder", "!!!").unwrap_err(),
            SessionError::Invalid("not base64")
        );
        assert_eq!(
            Credentials::new("modder", &STANDARD.encode(b"plain")).unwrap_err(),
            SessionError::Invalid("not gzip")
        );
        assert_eq!(
            Credentials::new("modder", &encode(b"")).unwrap_err(),
            SessionError::Invalid("empty")
        );
        let message = SessionError::MissingSession.to_string();
        assert!(message.contains("pdx-ws session export"), "{message}");
    }
}
