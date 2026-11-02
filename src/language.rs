//! Steam's API language codes, which name the language of a localized
//! title and description (spec sections 4 and 7, ADR 0003).

/// The API language codes of Steam's fully supported languages, from
/// <https://partner.steamgames.com/doc/store/localization/languages>. These
/// are the only languages the Steamworks API takes.
pub const API_LANGUAGES: [&str; 31] = [
    "arabic",
    "brazilian",
    "bulgarian",
    "czech",
    "danish",
    "dutch",
    "english",
    "finnish",
    "french",
    "german",
    "greek",
    "hungarian",
    "indonesian",
    "italian",
    "japanese",
    "koreana",
    "latam",
    "malay",
    "norwegian",
    "polish",
    "portuguese",
    "romanian",
    "russian",
    "schinese",
    "spanish",
    "swedish",
    "tchinese",
    "thai",
    "turkish",
    "ukrainian",
    "vietnamese",
];

/// The language of the descriptor's title and of `description`, which
/// Steam assumes when an update names none.
pub const DEFAULT: &str = "english";

/// Why `code` cannot name a localized text, or `None` when it can.
pub fn check(code: &str) -> Option<String> {
    if code == DEFAULT {
        return Some(
            "`english` is the default language: its title is the descriptor's name and its \
             description the `description` file"
                .to_owned(),
        );
    }
    if API_LANGUAGES.contains(&code) {
        return None;
    }
    let hint = match code.to_ascii_lowercase().as_str() {
        lower if lower != code && API_LANGUAGES.contains(&lower) => {
            format!("; Steam's codes are lowercase, e.g. `{lower}`")
        }
        "chinese" | "zh" | "zh-cn" | "zh_cn" | "zh-hans" => "; use `schinese`".to_owned(),
        "zh-tw" | "zh_tw" | "zh-hant" => "; use `tchinese`".to_owned(),
        "korean" | "ko" => "; use `koreana`".to_owned(),
        "ja" | "jp" => "; use `japanese`".to_owned(),
        "pt-br" | "pt_br" => "; use `brazilian`".to_owned(),
        "es-419" => "; use `latam`".to_owned(),
        _ => String::new(),
    };
    Some(format!(
        "`{code}` is not a Steam API language code{hint}; see \
         https://partner.steamgames.com/doc/store/localization/languages"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_are_sorted_and_unique() {
        assert!(API_LANGUAGES.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn known_codes_pass_and_english_does_not() {
        for code in ["schinese", "tchinese", "japanese", "koreana", "polish"] {
            assert_eq!(check(code), None, "{code}");
        }
        assert!(check("english").unwrap().contains("default language"));
    }

    #[test]
    fn unknown_codes_get_a_hint() {
        assert_eq!(
            check("korean").unwrap(),
            "`korean` is not a Steam API language code; use `koreana`; see \
             https://partner.steamgames.com/doc/store/localization/languages"
        );
        assert!(check("SChinese").unwrap().contains("e.g. `schinese`"));
        assert!(check("zh-CN").unwrap().contains("use `schinese`"));
        assert!(check("old").unwrap().starts_with("`old` is not a Steam"));
    }
}
