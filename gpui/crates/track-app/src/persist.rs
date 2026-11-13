//! Small UI-state file replacing the React client's localStorage keys.
use crate::core::geometry::DEFAULT_ZOOM;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistState {
    /// `track:calendar` zoomIndex
    pub zoom_index: usize,
    /// `reports.expandedPaths`, keyed by groupBy
    pub reports_expanded: HashMap<String, Vec<String>>,
    /// `reports.groupSameTitles`
    pub group_same_titles: bool,
    /// `reports.customOutputCache.v1`
    pub output_cache: Vec<track_core::report::cache::CachedOutput>,
    /// How report durations are shown.
    pub duration_style: DurationStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DurationStyle {
    /// "1.5h", matching the decimal hours typed into timesheets.
    #[default]
    Decimal,
    /// "1h 30m"
    HoursMinutes,
}

impl Default for PersistState {
    fn default() -> Self {
        Self { zoom_index: DEFAULT_ZOOM, reports_expanded: HashMap::new(), group_same_titles: false, output_cache: Vec::new(), duration_style: DurationStyle::Decimal }
    }
}

impl PersistState {
    pub fn default_path() -> PathBuf {
        dirs::home_dir().unwrap_or_default().join(".track-gpui").join("gpui-prototype.json")
    }

    /// Missing or unreadable files yield the defaults.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(self).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_persist_file_with_summary_collapsed_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.json");
        std::fs::write(&path, r#"{"summary_collapsed": true, "zoom_index": 2}"#).unwrap();
        let s = PersistState::load(&path);
        assert_eq!(s.zoom_index, 2, "the rest of an old file still loads");
        assert!(!s.group_same_titles);
        let saved = { s.save(&path).unwrap(); std::fs::read_to_string(&path).unwrap() };
        assert!(!saved.contains("summary_collapsed"), "the dropped setting is not written back");
    }

    #[test]
    fn persist_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/gpui-prototype.json");
        let mut s = PersistState::default();
        assert_eq!(s.zoom_index, 1);
        s.zoom_index = 2;
        s.group_same_titles = true;
        s.reports_expanded.insert("project".into(), vec!["project:a".into()]);
        s.save(&path).unwrap();
        assert_eq!(PersistState::load(&path), s);
    }

    #[test]
    fn persist_corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(PersistState::load(&path), PersistState::default());
        assert_eq!(PersistState::load(&dir.path().join("missing.json")), PersistState::default());
    }
}
