//! Usage: Notification-related Tauri commands.

use crate::notice;

#[tauri::command]
pub(crate) fn notice_send(
    app: tauri::AppHandle,
    level: notice::NoticeLevel,
    title: Option<String>,
    body: String,
) -> Result<bool, String> {
    notice::emit(&app, notice::build(level, title, body))?;
    Ok(true)
}
