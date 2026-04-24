use winapi::um::wingdi::{GetDeviceGammaRamp, SetDeviceGammaRamp};
use winapi::um::winuser::{GetDC, ReleaseDC};
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
        let hwnd = ptr::null_mut();
        let dc = GetDC(hwnd);
        if dc.is_null() {
            return Err(format!("Failed to get DC (error: {})", GetLastError()));
        }

        // First, read the current gamma ramp as a baseline
        let mut current_ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };
        GetDeviceGammaRamp(dc, &mut current_ramp as *mut GammaRamp as *mut _);

        // Build the identity ramp (what "default" looks like)
        // Then apply our adjustments on top
        let brightness = (settings.brightness - 50.0) / 100.0 * 0.5; // -0.25 to +0.25
        let contrast = 0.6 + (settings.contrast / 100.0) * 0.8;      // 0.6 to 1.4
        let gamma = settings.gamma.max(0.3).min(2.8);

        let mut new_ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };

        for i in 0..256 {
            let normalized = i as f32 / 255.0;

            // Apply gamma
            let g = normalized.powf(1.0 / gamma);

            // Apply contrast
            let c = ((g - 0.5) * contrast + 0.5).max(0.0).min(1.0);

            // Apply brightness
            let b = (c + brightness).max(0.0).min(1.0);

            let val = (b * 65535.0) as u16;

            // Ensure value doesn't deviate more than ~50% from identity
            let identity = (i as u32 * 257) as u16; // identity ramp: 0, 257, 514, ...
            let max_dev: i32 = 32768;
            let clamped = (val as i32)
                .max(identity as i32 - max_dev)
                .min(identity as i32 + max_dev)
                .max(0)
                .min(65535) as u16;

            // Ensure monotonically increasing
            let final_val = if i > 0 {
                clamped.max(new_ramp.red[i - 1])
            } else {
                clamped
            };

            new_ramp.red[i] = final_val;
            new_ramp.green[i] = final_val;
            new_ramp.blue[i] = final_val;
        }

        let result = SetDeviceGammaRamp(dc, &mut new_ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        ReleaseDC(hwnd, dc);

        if result == 0 {
            return Err(format!("Failed to set gamma ramp (error: {}, brightness: {}, contrast: {}, gamma: {})",
                err, settings.brightness, settings.contrast, settings.gamma));
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
                        "quit" => { app.exit(0); }
                        "preset_gaming" => { let _ = app.emit("set-preset", "gaming"); }
                        "preset_cinema" => { let _ = app.emit("set-preset", "cinema"); }
                        "preset_default" => { let _ = app.emit("set-preset", "default"); }
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
        .invoke_handler(tauri::generate_handler![apply_color_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
