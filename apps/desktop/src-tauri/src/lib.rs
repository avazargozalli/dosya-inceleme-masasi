mod commands;
mod error;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Kasa durumu süreç ömrü boyunca tek örnektir; ikinci pencere ayrı
            // DB yazıcısı açmaz (ADR-02, 06.5).
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault_create,
            commands::vault_open,
            commands::vault_open_with_recovery,
            commands::vault_lock,
            commands::vault_status,
            commands::system_info,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri uygulaması başlatılamadı");
}
