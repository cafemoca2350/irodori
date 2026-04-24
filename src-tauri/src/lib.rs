use winapi::um::wingdi::SetDeviceGammaRamp;
use winapi::um::winuser::GetDC;
use serde::Deserialize;
use tauri::{Emitter, Manager};
use std::ptr;

#[derive(Deserialize)]
struct ColorSettings {
    brightness: f32,  // 0-100, default 50
    contrast: f32,    // 0-100, default 50
    gamma: f32,       // 0.5-2.0, default 1.0
    vibrance: f32,    // 0-100, default 50
    hue: f32,         // 0-360, default 0
}

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let dc = GetDC(ptr::null_mut());
        if dc.is_null() {
            return Err("Failed to get DC".to_string());
        }

        // Normalize parameters
        let brightness = (settings.brightness - 50.0) / 100.0; // -0.5 to 0.5
        let contrast = settings.contrast / 50.0;                // 0.0 to 2.0
        let gamma = settings.gamma;                              // 0.5 to 2.0

        let mut ramp = [0u16; 768];
        for i in 0..256 {
            let normalized = i as f32 / 255.0;

            // Apply gamma correction
            let gamma_corrected = if gamma <= 0.0 { 0.0 } else { normalized.powf(1.0 / gamma) };

            // Apply contrast (scale around midpoint)
            let contrasted = ((gamma_corrected - 0.5) * contrast) + 0.5;

            // Apply brightness (offset)
            let final_val = contrasted + brightness;

            // Clamp and convert to u16
            let clamped = final_val.max(0.0).min(1.0);
            let val = (clamped * 65535.0) as u16;

            // For now, apply same curve to all channels
            // (hue and vibrance would need per-channel adjustment)
            ramp[i] = val;         // R
            ramp[i + 256] = val;   // G
            ramp[i + 512] = val;   // B
        }

        if SetDeviceGammaRamp(dc, ramp.as_mut_ptr() as *mut _) == 0 {
            return Err("Failed to set gamma ramp".to_string());
        }
    }
    Ok(())
}

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let gaming = MenuItem::with_id(app, "preset_gaming", "Gaming", true, None::<&str>)?;
            let cinema = MenuItem::with_id(app, "preset_cinema", "Cinema", true, None::<&str>)?;
            let default = MenuItem::with_id(app, "preset_default", "Default", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&gaming, &cinema, &default, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "preset_gaming" => {
                            let _ = app.emit("set-preset", "gaming");
                        }
                        "preset_cinema" => {
                            let _ = app.emit("set-preset", "cinema");
                        }
                        "preset_default" => {
                            let _ = app.emit("set-preset", "default");
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            apply_color_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
