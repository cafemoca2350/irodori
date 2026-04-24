use winapi::um::wingdi::{CreateDCA, DeleteDC, SetDeviceGammaRamp};
use winapi::um::errhandlingapi::GetLastError;
use serde::Deserialize;
use tauri::{Emitter, Manager};
use std::ptr;

#[repr(C)]
struct GammaRamp {
    red: [u16; 256],
    green: [u16; 256],
    blue: [u16; 256],
}

#[derive(Deserialize)]
struct ColorSettings {
    brightness: f32,
    contrast: f32,
    gamma: f32,
    vibrance: f32,
    hue: f32,
}

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        // Get DC for the primary display device directly
        let dc = CreateDCA(
            b"DISPLAY\0".as_ptr() as *const i8,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        );
        if dc.is_null() {
            let err = GetLastError();
            return Err(format!("Failed to get display DC (error: {})", err));
        }

        // Normalize parameters
        let brightness = (settings.brightness - 50.0) / 50.0 * 0.3;
        let contrast = 0.5 + (settings.contrast / 100.0);
        let gamma = settings.gamma.max(0.3).min(2.8);

        let mut ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };

        let mut prev: u16 = 0;
        for i in 0..256 {
            let normalized = i as f32 / 255.0;

            // Gamma correction
            let gamma_val = normalized.powf(1.0 / gamma);

            // Contrast (scale around midpoint)
            let contrast_val = (gamma_val - 0.5) * contrast + 0.5;

            // Brightness (offset)
            let final_val = (contrast_val + brightness).max(0.0).min(1.0);

            let raw = (final_val * 65535.0) as u16;

            // Ensure monotonically increasing
            let val = raw.max(prev);

            ramp.red[i] = val;
            ramp.green[i] = val;
            ramp.blue[i] = val;

            prev = val;
        }

        let result = SetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        let last_err = GetLastError();

        // Always clean up the DC
        DeleteDC(dc);

        if result == 0 {
            return Err(format!("Failed to set gamma ramp (error: {})", last_err));
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
