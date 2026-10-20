//! Reading steamcmd's output: terminal codes, masking, and the result
//! signals that experiment E4 recorded (spec section 9).

use crate::item_id::ItemId;

/// Removes ANSI escape sequences, which steamcmd prints on Linux.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            // Parameters and intermediates, then one final byte in `@`..=`~`.
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        }
    }
    out
}

/// Masks what ties a log to the Steam account: SteamID3s such as
/// `[U:1:12345]` become `[U:1:***]`, and the quoted user name becomes
/// `'***'`.
pub fn mask(line: &str, user: &str) -> String {
    const PREFIX: &str = "[U:1:";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find(PREFIX) {
        let after = &rest[start + PREFIX.len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        out.push_str(&rest[..start]);
        if digits > 0 && after[digits..].starts_with(']') {
            out.push_str("[U:1:***]");
            rest = &after[digits + 1..];
        } else {
            out.push_str(PREFIX);
            rest = after;
        }
    }
    out.push_str(rest);
    if user.is_empty() {
        out
    } else {
        out.replace(&format!("'{user}'"), "'***'")
    }
}

/// What a `workshop_build_item` run says about the publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// `Committing update...Success.` with exit code 0. `created` is the ID
    /// from `Create new workshop item ( PublishFileID <id>)`.
    Success { created: Option<ItemId> },
    /// Steam refused the cached login: exit 5 with `Cached credentials not
    /// found` or `Invalid Password`, or a Steam Guard prompt.
    SessionRejected { reason: String },
    /// Exit 9, e.g. `ERROR! Build for workshop item has no content`.
    BuildFailed,
    /// Anything else, including exit 0 without the success line.
    Failed,
}

/// Classifies a run from its exit code and its ANSI-free output.
pub fn classify(code: Option<i32>, output: &str) -> Signal {
    let guard = ["Steam Guard code:", "Two-factor code:"]
        .into_iter()
        .find(|prompt| output.contains(prompt));
    if let Some(prompt) = guard {
        return Signal::SessionRejected {
            reason: format!(
                "steamcmd asked for a code (`{}`)",
                prompt.trim_end_matches(':')
            ),
        };
    }
    if code == Some(5) {
        for reason in ["Cached credentials not found", "Invalid Password"] {
            if output.contains(reason) {
                return Signal::SessionRejected {
                    reason: reason.to_owned(),
                };
            }
        }
    }
    match code {
        Some(0) if output.contains("Committing update...Success.") => Signal::Success {
            created: created_id(output),
        },
        Some(9) => Signal::BuildFailed,
        _ => Signal::Failed,
    }
}

fn created_id(output: &str) -> Option<ItemId> {
    const MARKER: &str = "Create new workshop item ( PublishFileID ";
    let start = output.find(MARKER)? + MARKER.len();
    let digits: String = output[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// The lines worth showing when a run fails: steamcmd's own `ERROR` lines.
pub fn error_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("ERROR") || line.contains("FAILED"))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_terminal_codes() {
        let line =
            "Committing update...\u{1b}[0mSuccess.\u{1b}[0mUnloading Steam API...\u{1b}[1mOK";
        assert_eq!(
            strip_ansi(line),
            "Committing update...Success.Unloading Steam API...OK"
        );
    }

    #[test]
    fn masks_steam_ids_and_the_user_name() {
        assert_eq!(
            mask(
                "Logging in user 'modder' [U:1:123456789] to Steam Public...OK",
                "modder"
            ),
            "Logging in user '***' [U:1:***] to Steam Public...OK"
        );
        assert_eq!(
            mask("[U:1:] [U:1:12x] [U:1:7]", ""),
            "[U:1:] [U:1:12x] [U:1:***]"
        );
    }

    #[test]
    fn classifies_the_e4_signals() {
        let created = "Create new workshop item ( PublishFileID 3823257732).\n\nCommitting update...Success.Unloading Steam API...OK";
        assert_eq!(
            classify(Some(0), created),
            Signal::Success {
                created: Some("3823257732".parse().unwrap())
            }
        );
        assert_eq!(
            classify(Some(0), "Committing update...Success."),
            Signal::Success { created: None }
        );
        assert_eq!(
            classify(Some(0), "Unloading Steam API...OK"),
            Signal::Failed
        );
        assert_eq!(
            classify(Some(9), "ERROR! Build for workshop item has no content"),
            Signal::BuildFailed
        );
        assert_eq!(
            classify(
                Some(5),
                "Cached credentials not found.\nERROR (Invalid Password)"
            ),
            Signal::SessionRejected {
                reason: "Cached credentials not found".to_owned()
            }
        );
        assert_eq!(
            classify(Some(5), "ERROR (Rate Limit Exceeded)"),
            Signal::Failed
        );
        assert!(matches!(
            classify(Some(0), "Steam Guard code:"),
            Signal::SessionRejected { .. }
        ));
    }

    #[test]
    fn error_lines_are_kept() {
        let output = "[2026-10-20 01:41:38]: ERROR! Build for workshop item has no content\nERROR! Failed to update workshop item (Failure).\nUnloading Steam API...OK";
        assert_eq!(
            error_lines(output),
            [
                "[2026-10-20 01:41:38]: ERROR! Build for workshop item has no content",
                "ERROR! Failed to update workshop item (Failure)."
            ]
        );
    }
}
