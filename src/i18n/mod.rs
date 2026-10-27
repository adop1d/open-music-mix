//! Embedded English/Spanish tables with per-key user overrides.

use std::collections::HashMap;
use std::path::Path;

pub const ENGLISH: &str = include_str!("en.toml");
pub const SPANISH: &str = include_str!("es.toml");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
        }
    }

    pub fn from_preference(value: Option<&str>) -> Self {
        if let Some(value) = value {
            if value.to_lowercase().starts_with("es") {
                return Language::Spanish;
            }
            if value.to_lowercase().starts_with("en") {
                return Language::English;
            }
        }
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(key) {
                if value.to_lowercase().starts_with("es") {
                    return Language::Spanish;
                }
                if !value.is_empty() {
                    return Language::English;
                }
            }
        }
        Language::English
    }
}

#[derive(Clone, Debug)]
pub struct I18n {
    language: Language,
    english: HashMap<String, String>,
    active: HashMap<String, String>,
}

impl Default for I18n {
    fn default() -> Self {
        Self::english()
    }
}

impl I18n {
    pub fn english() -> Self {
        Self::from_tables(ENGLISH, SPANISH, Language::English, None)
    }

    pub fn load(preference: Option<&str>, config_dir: Option<&Path>) -> (Self, Vec<String>) {
        let language = Language::from_preference(preference);
        let overrides = config_dir
            .map(|dir| dir.join("i18n").join(format!("{}.toml", language.code())))
            .filter(|path| path.exists());
        let mut warnings = Vec::new();
        let (mut i18n, mut parse_warnings) = if language == Language::English {
            (
                Self::from_tables(ENGLISH, SPANISH, Language::English, None),
                Vec::new(),
            )
        } else {
            (
                Self::from_tables(ENGLISH, SPANISH, Language::Spanish, None),
                Vec::new(),
            )
        };
        warnings.append(&mut parse_warnings);
        if let Some(path) = overrides {
            match std::fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<HashMap<String, String>>(&text) {
                    Ok(map) => {
                        i18n.active.extend(map);
                    }
                    Err(_) => warnings.push(format!(
                        "{}: {}",
                        i18n.t("msg_i18n_invalid"),
                        path.display()
                    )),
                },
                Err(_) => warnings.push(format!(
                    "{}: {}",
                    i18n.t("msg_i18n_invalid"),
                    path.display()
                )),
            }
        }
        (i18n, warnings)
    }

    pub fn from_tables(
        english: &str,
        spanish: &str,
        language: Language,
        _override_path: Option<&Path>,
    ) -> Self {
        let english = parse_table(english);
        let mut active = english.clone();
        active.extend(parse_table(spanish));
        let mut i18n = Self {
            language,
            english,
            active,
        };
        if language == Language::English {
            i18n.active = i18n.english.clone();
        }
        i18n
    }

    pub fn for_test(english: &str, spanish: &str, language: Language) -> Self {
        Self::from_tables(english, spanish, language, None)
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    /// Active language, then English, then the key itself.
    pub fn t(&self, key: &str) -> String {
        let source = if self.language == Language::Spanish {
            &self.active
        } else {
            &self.english
        };
        source
            .get(key)
            .or_else(|| self.english.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

fn parse_table(text: &str) -> HashMap<String, String> {
    toml::from_str(text).unwrap_or_default()
}
