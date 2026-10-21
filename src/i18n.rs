//! Minimal embedded i18n: English, Brazilian Portuguese and Simplified Chinese.
//!
//! The original application ships nine locales in its remote renderer; this native
//! build embeds the three most relevant ones and falls back to English for any
//! missing key. Dictionary files live in `assets/i18n/*.json`.

use std::collections::HashMap;

/// Available UI languages: (code, native label, dictionary JSON).
const LANGUAGES: &[(&str, &str, &str)] = &[
    ("en", "English", include_str!("../assets/i18n/en-US.json")),
    ("pt", "Português (Brasil)", include_str!("../assets/i18n/pt-BR.json")),
    ("zh", "简体中文", include_str!("../assets/i18n/zh-CN.json")),
];

type Dict = HashMap<String, String>;

/// Translator holding the loaded dictionaries.
pub struct I18n {
    lang: String,
    dict: Dict,
    fallback: Dict,
}

impl I18n {
    /// Build a translator for the requested language or the OS language.
    pub fn new(requested: Option<&str>, system: &str) -> Self {
        let lang = normalize_lang(requested.unwrap_or(system));
        let fallback = load_dict("en").unwrap_or_default();
        let dict = if lang == "en" {
            fallback.clone()
        } else {
            load_dict(&lang).unwrap_or_default()
        };
        log::info!("i18n: language = {lang} (requested {:?}, system {system})", requested);
        Self {
            lang,
            dict,
            fallback,
        }
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// All selectable languages (code, native label).
    pub fn available() -> impl Iterator<Item = (&'static str, &'static str)> {
        LANGUAGES.iter().map(|(code, label, _)| (*code, *label))
    }

    /// Translate a key, falling back to English and then to the key itself.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(value) = self.dict.get(key) {
            return value;
        }
        if let Some(value) = self.fallback.get(key) {
            return value;
        }
        key
    }

    /// Translate a key with `{name}` placeholders substituted.
    pub fn t_args(&self, key: &str, args: &[(&str, String)]) -> String {
        let mut out = self.t(key).to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

fn normalize_lang(tag: &str) -> String {
    let short = tag
        .split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .to_ascii_lowercase();
    match short.as_str() {
        "zh" => "zh".into(),
        "pt" => "pt".into(),
        "en" => "en".into(),
        other if LANGUAGES.iter().any(|(code, _, _)| *code == other) => other.into(),
        _ => "en".into(),
    }
}

fn load_dict(code: &str) -> Option<Dict> {
    let json = LANGUAGES
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, _, json)| *json)?;
    match serde_json::from_str(json) {
        Ok(dict) => Some(dict),
        Err(err) => {
            log::error!("failed to parse i18n dictionary {code}: {err}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_and_translations_exist() {
        let i18n = I18n::new(Some("pt-BR"), "en-US");
        assert_eq!(i18n.lang(), "pt");
        assert_eq!(i18n.t("nav.eq"), "Equalizador");
        assert_eq!(i18n.t("missing.key"), "missing.key");
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        let i18n = I18n::new(Some("ru-RU"), "ru-RU");
        assert_eq!(i18n.lang(), "en");
        assert_eq!(i18n.t("nav.eq"), "Equalizer");
    }
}
