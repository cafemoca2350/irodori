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
    vibrance: f32,    // 0-100, default 50 (reserved)
    hue: f32,         // 0-360, default 0 (reserved)
}

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let dc = GetDC(ptr::null_mut());
        if dc.is_null() {
            return Err("Failed to get DC".to_string());
        }

        // Normalize parameters to safe ranges
        // Brightness: map 0-100 to -0.3..+0.3 (conservative to stay within Windows limits)
        let brightness = (settings.brightness - 50.0) / 50.0 * 0.3;
        // Contrast: map 0-100 to 0.5..1.5
        let contrast = 0.5 + (settings.contrast / 100.0);
        // Gamma: 0.5..2.0, used directly
        let gamma = settings.gamma.max(0.3).min(2.8);

        let mut ramp = [0u16; 768];
        let mut prev_r: u16 = 0;
        let mut prev_g: u16 = 0;
        let mut prev_b: u16 = 0;

        for i in 0..256 {
            let normalized = i as f32 / 255.0;

            // Step 1: Apply gamma correction
            let gamma_val = normalized.powf(1.0 / gamma);

            // Step 2: Apply contrast (scale around 0.5)
            let contrast_val = (gamma_val - 0.5) * contrast + 0.5;

            // Step 3: Apply brightness (offset)
            let final_val = (contrast_val + brightness).max(0.0).min(1.0);

            // Convert to u16, clamping to safe range
            let raw = (final_val * 65535.0) as u16;

            // Ensure monotonically increasing (Windows requirement)
            let r = raw.max(prev_r);
            let g = raw.max(prev_g);
            let b = raw.max(prev_b);

            ramp[i] = r;
            ramp[i + 256] = g;
            ramp[i + 512] = b;

            prev_r = r;
            prev_g = g;
            prev_b = b;
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
