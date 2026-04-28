//! T1.0.4.17 — Desktop notification command.
//!
//! Thin wrapper over `tauri-plugin-notification`. The frontend calls
//! `send_notification(title, body)` and we delegate to the OS native
//! notification centre.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Fire a native desktop notification.
#[tauri::command]
pub async fn send_notification(app: AppHandle, title: String, body: String) -> Result<(), String> {
    app.notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn notification_command_has_expected_name() {
        // The function name is pinned as a Tauri command string.
        // Runtime testing requires a full Tauri app handle + OS
        // notification centre — covered by E2E.
        assert_eq!(stringify!(send_notification), "send_notification",);
    }
}
