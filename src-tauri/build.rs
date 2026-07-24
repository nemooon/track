fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new()
                .commands(&["generate_weekly_report", "show_ai_integration_installer"]),
        ),
    )
    .expect("failed to run tauri build script");
}
