//! Dependency-free internationalisation.
//!
//! Translations live in `locales/<code>.txt` (`key = value` lines, `#` comments,
//! `\n` escape) and are embedded in the executable. English is the fallback for
//! any missing key. To add a language: create the file, then add a [`Lang`]
//! variant, its code and its `include_str!` below.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Fr,
    De,
    Es,
    It,
    Pt,
    Uk,
}

impl Lang {
    pub const ALL: [Lang; 7] = [Lang::En, Lang::Fr, Lang::De, Lang::Es, Lang::It, Lang::Pt, Lang::Uk];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::It => "it",
            Lang::Pt => "pt",
            Lang::Uk => "uk",
        }
    }

    /// Name of the language in that language (shown in the language selector).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::It => "Italiano",
            Lang::Pt => "Português",
            Lang::Uk => "Українська (UKR)",
        }
    }

    /// Accepts `fr`, `fr-FR`, `fr_CA`, `FR`... (matches on the primary subtag).
    pub fn from_code(code: &str) -> Option<Lang> {
        let primary = code.split(['-', '_', '.']).next()?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == primary)
    }

    fn source(self) -> &'static str {
        match self {
            Lang::En => include_str!("../locales/en.txt"),
            Lang::Fr => include_str!("../locales/fr.txt"),
            Lang::De => include_str!("../locales/de.txt"),
            Lang::Es => include_str!("../locales/es.txt"),
            Lang::It => include_str!("../locales/it.txt"),
            Lang::Pt => include_str!("../locales/pt.txt"),
            Lang::Uk => include_str!("../locales/uk.txt"),
        }
    }
}

#[derive(Clone)]
pub struct Translator {
    lang: Lang,
    current: HashMap<&'static str, String>,
    fallback: HashMap<&'static str, String>,
}

impl Translator {
    pub fn new(lang: Lang) -> Self {
        Translator { lang, current: parse(lang.source()), fallback: parse(Lang::En.source()) }
    }

    /// Language chosen by `--lang` / `YCOPY_LANG`, else the Windows user locale, else English.
    pub fn detect(explicit: Option<&str>) -> Self {
        let lang = explicit
            .map(str::to_owned)
            .or_else(|| std::env::var("YCOPY_LANG").ok())
            .or_else(crate::platform::user_locale)
            .and_then(|c| Lang::from_code(&c))
            .unwrap_or(Lang::En);
        Translator::new(lang)
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Translated text for `key` (falls back to English, then to the key itself).
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.current
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }

    /// Like [`t`](Self::t), replacing `{0}`, `{1}`... with `args`.
    pub fn f(&self, key: &str, args: &[&str]) -> String {
        let mut text = self.t(key).to_owned();
        for (i, arg) in args.iter().enumerate() {
            text = text.replace(&format!("{{{i}}}"), arg);
        }
        text
    }

    pub fn decimal_separator(&self) -> &str {
        self.t("decimal_separator")
    }

    /// Human readable message for a copy error.
    pub fn copy_error(&self, e: &crate::copier::CopyError) -> String {
        use crate::copier::CopyError::*;
        match e {
            Io { path, source } => self.f("error.io", &[&path.display().to_string(), &source.to_string()]),
            SameFile(p) => self.f("error.same_file", &[&p.display().to_string()]),
            NoFileName(p) => self.f("error.no_file_name", &[&p.display().to_string()]),
            Cancelled => self.t("status.cancelled").to_owned(),
        }
    }
}

fn parse(source: &'static str) -> HashMap<&'static str, String> {
    source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim().replace("\\n", "\n")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_defines_every_english_key() {
        let english = parse(Lang::En.source());
        for lang in Lang::ALL {
            let map = parse(lang.source());
            for key in english.keys() {
                assert!(map.contains_key(key), "{}: missing key `{key}`", lang.code());
            }
            for key in map.keys() {
                assert!(english.contains_key(key), "{}: unknown key `{key}`", lang.code());
            }
        }
    }

    #[test]
    fn placeholders_match_english() {
        let placeholders = |s: &str| {
            let mut v: Vec<String> = (0..6).map(|i| format!("{{{i}}}")).filter(|p| s.contains(p)).collect();
            v.sort();
            v
        };
        let english = parse(Lang::En.source());
        for lang in Lang::ALL {
            for (key, value) in parse(lang.source()) {
                assert_eq!(placeholders(&value), placeholders(&english[key]), "{}: `{key}`", lang.code());
            }
        }
    }

    #[test]
    fn language_codes() {
        assert_eq!(Lang::from_code("fr-FR"), Some(Lang::Fr));
        assert_eq!(Lang::from_code("pt_BR"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("DE"), Some(Lang::De));
        assert_eq!(Lang::from_code("ja-JP"), None);
    }
}
