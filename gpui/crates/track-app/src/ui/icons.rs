//! Icons beyond gpui-component's built-in set, served next to its assets.
use gpui::{AssetSource, SharedString};

gpui_kit_assets::icon_assets!(ExtraIcons, [Pipette, Archive, ArchiveRestore]);

/// gpui-component's assets plus the extra Lucide icons this app uses.
pub struct AppAssets(pub gpui_kit_assets::Assets);

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        self.0.load(path)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut paths = self.0.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

/// The eyedropper (Lucide `pipette`).
pub fn pipette() -> gpui_component::Icon {
    gpui_component::Icon::empty().path(gpui_kit_assets::IconName::Pipette.path())
}

/// Archive (Lucide `archive`).
pub fn archive() -> gpui_component::Icon {
    gpui_component::Icon::empty().path(gpui_kit_assets::IconName::Archive.path())
}

/// Restore from the archive (Lucide `archive-restore`).
pub fn archive_restore() -> gpui_component::Icon {
    gpui_component::Icon::empty().path(gpui_kit_assets::IconName::ArchiveRestore.path())
}
