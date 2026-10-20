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
    /// found` or `Invalid Password`, `License expired`, or a Steam Guard
    /// prompt.
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
    // Issue #9 names `License expired` as a dead session too.
    if output.contains("License expired") {
        return Signal::SessionRejected {
            reason: "License expired".to_owned(),
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

/// Whether a successful `workshop_build_item` made a new version, from
/// steamcmd's logs. stdout is the same either way (issue #39).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentChange {
    /// `Uploaded new content ( ManifestID … )` (`workshop_log.txt`) or
    /// `Success! New manifestID … created` (`depot_build_<appid>.log`).
    New,
    /// `No content change detected for item …` or `Success! Previous
    /// manifestID … reused, no chunks uploaded.`: Steam kept the item's
    /// version and did not record the change note.
    Unchanged,
    /// Neither, e.g. the logs are missing.
    Unknown,
}

/// Reads [`ContentChange`] from the text of the steamcmd logs.
pub fn content_change(logs: &str) -> ContentChange {
    let unchanged = logs.contains("No content change detected")
        || logs
            .lines()
            .any(|line| line.contains("Previous manifestID") && line.contains("reused"));
    if unchanged {
        ContentChange::Unchanged
    } else if logs.contains("Uploaded new content") || logs.contains("New manifestID") {
        ContentChange::New
    } else {
        ContentChange::Unknown
    }
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
        assert_eq!(
            classify(Some(5), "ERROR (License expired)"),
            Signal::SessionRejected {
                reason: "License expired".to_owned()
            }
        );
        assert!(matches!(
            classify(Some(0), "Steam Guard code:"),
            Signal::SessionRejected { .. }
        ));
    }

    #[test]
    fn reads_whether_steam_made_a_new_version() {
        // The logs of E2E run 39497323643: new content, then the same again.
        let new = "workshop_log.txt: [2026-10-20 21:07:07] [AppID 236850] Uploaded new content ( ManifestID 4390425272313943695 ) for item 3823619285.\n\
                   depot_build_236850.log: [2026-10-20 21:07:07]: Success! New manifestID 4390425272313943695 created and 1 new chunks uploaded.";
        let same = "workshop_log.txt: [2026-10-20 21:07:44] [AppID 236850] No content change detected for item 3823619285\n\
                    depot_build_236850.log: [2026-10-20 21:07:44]: Success! Previous manifestID 4390425272313943695 reused, no chunks uploaded.";
        assert_eq!(content_change(new), ContentChange::New);
        assert_eq!(content_change(same), ContentChange::Unchanged);
        // Either log alone is enough.
        assert_eq!(
            content_change(same.lines().nth(1).unwrap()),
            ContentChange::Unchanged
        );
        assert_eq!(
            content_change(new.lines().next().unwrap()),
            ContentChange::New
        );
        assert_eq!(content_change(""), ContentChange::Unknown);
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
