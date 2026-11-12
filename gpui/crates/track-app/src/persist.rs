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
    /// `reports.summaryCollapsed`
    pub summary_collapsed: bool,
    /// `reports.customOutputCache.v1`
    pub output_cache: Vec<track_core::report::cache::CachedOutput>,
}

impl Default for PersistState {
    fn default() -> Self {
        Self { zoom_index: DEFAULT_ZOOM, reports_expanded: HashMap::new(), group_same_titles: false, summary_collapsed: false, output_cache: Vec::new() }
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
