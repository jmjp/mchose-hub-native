//! Equalizer preset database.
//!
//! The factory presets are ported from the shipped `eq.json` of MCHOSE HUB
//! (`%APPDATA%\MCHOSEHUB\files\config\eq.json`), which stores 10 integer band
//! gains per preset, grouped into "game" and "music" categories.

use serde::{Deserialize, Serialize};

/// Embedded factory preset file (see `assets/presets.json`).
const FACTORY_JSON: &str = include_str!("../../assets/presets.json");

/// Localized display name for a preset.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalizedNames {
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub zh: String,
    #[serde(default)]
    pub pt: String,
}

impl LocalizedNames {
    /// Resolve a name for the given UI language, falling back to English.
    pub fn for_lang(&self, lang: &str) -> &str {
        let candidate = if lang.starts_with("zh") {
            &self.zh
        } else if lang.starts_with("pt") {
            &self.pt
        } else {
            &self.en
        };
        if candidate.is_empty() {
            &self.en
        } else {
            candidate
        }
    }
}

/// One equalizer preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub id: String,
    #[serde(default)]
    pub group: Option<String>,
    pub names: LocalizedNames,
    /// Ten band gains in dB (legacy C-Media scale).
    pub data: Vec<i32>,
}

impl EqPreset {
    /// Band gains padded/truncated to exactly [`crate::model::audio::EQ_BANDS`].
    pub fn bands(&self) -> [i32; crate::model::audio::EQ_BANDS] {
        let mut out = [0i32; crate::model::audio::EQ_BANDS];
        for (i, v) in self.data.iter().take(out.len()).enumerate() {
            out[i] = *v;
        }
        out
    }

    pub fn display_name(&self, lang: &str) -> &str {
        self.names.for_lang(lang)
    }
}

/// A preset category ("game" / "music").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCategory {
    pub id: String,
    pub name: LocalizedNames,
    pub presets: Vec<EqPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FactoryFile {
    version: u32,
    #[serde(default)]
    source: String,
    categories: Vec<PresetCategory>,
}

/// The preset database: factory presets plus user-created ones.
#[derive(Debug, Clone, Default)]
pub struct PresetDb {
    pub factory: Vec<PresetCategory>,
    pub user: Vec<EqPreset>,
    /// Currently selected preset id (factory or user).
    pub selected: Option<String>,
}

impl PresetDb {
    /// Load the embedded factory presets.
    pub fn load_factory() -> Self {
        match serde_json::from_str::<FactoryFile>(FACTORY_JSON) {
            Ok(file) => Self {
                factory: file.categories,
                user: Vec::new(),
                selected: None,
            },
            Err(err) => {
                log::error!("failed to parse embedded presets.json: {err}");
                Self::default()
            }
        }
    }

    /// All presets, factory first.
    pub fn iter(&self) -> impl Iterator<Item = &EqPreset> {
        self.factory
            .iter()
            .flat_map(|c| c.presets.iter())
            .chain(self.user.iter())
    }

    pub fn find(&self, id: &str) -> Option<&EqPreset> {
        self.iter().find(|p| p.id == id)
    }

    pub fn selected_preset(&self) -> Option<&EqPreset> {
        self.selected.as_deref().and_then(|id| self.find(id))
    }

    /// Insert or replace a user preset.
    pub fn upsert_user(&mut self, preset: EqPreset) {
        if let Some(existing) = self.user.iter_mut().find(|p| p.id == preset.id) {
            *existing = preset;
        } else {
            self.user.push(preset);
        }
    }

    pub fn delete_user(&mut self, id: &str) -> bool {
        let before = self.user.len();
        self.user.retain(|p| p.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        self.user.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_presets_parse_and_are_ten_band() {
        let db = PresetDb::load_factory();
        assert!(!db.factory.is_empty(), "factory categories present");
        let count = db.iter().count();
        assert!(count >= 20, "expected the shipped preset set, got {count}");
        for preset in db.iter() {
            assert_eq!(preset.bands().len(), 10);
            assert!(!preset.names.en.is_empty());
        }
    }

    #[test]
    fn localized_names_fall_back() {
        let mut names = LocalizedNames::default();
        names.en = "Default".into();
        assert_eq!(names.for_lang("zh-CN"), "Default");
        names.zh = "默认".into();
        assert_eq!(names.for_lang("zh-CN"), "默认");
    }
}
