use std::path::PathBuf;
use tauri::Manager;

pub const APP_DOTDIR_NAME: &str = ".aio-coding-hub";

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let home_dir = app
        .path()
        .home_dir()
        .map_err(|e| format!("failed to resolve home dir: {e}"))?;

    let dir = home_dir.join(APP_DOTDIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create app dir: {e}"))?;

    Ok(dir)
}
