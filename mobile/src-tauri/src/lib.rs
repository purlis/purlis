//! purlis on your phone: the Tauri shell over `purlis_mobile_core` (iOS design §3). The
//! screens call the commands in `commands`; the state they share is one `Phone`.

mod commands;
mod phone;

use std::sync::Arc;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // The scanner and the lock are the phone's: a desktop build (for development) pastes the
    // pairing text and has no lock.
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_barcode_scanner::init())
        .plugin(tauri_plugin_biometric::init());
    builder
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let phone = phone::Phone::open(&data).map_err(|err| err.to_string())?;
            app.manage(Arc::new(phone));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::phone,
            commands::rename_phone,
            commands::desktops,
            commands::pair,
            commands::rename_desktop,
            commands::set_addresses,
            commands::unpair,
            commands::connect,
            commands::reconnect_all,
            commands::standing,
            commands::list,
            commands::attach,
            commands::ack,
            commands::detach,
            commands::attach_timeline,
            commands::detach_timeline,
            commands::write,
            commands::prompt,
            commands::answer,
            commands::stop,
            commands::forget_link_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running purlis");
}
