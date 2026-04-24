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
    brightness: f32,  // 0-100, default 50
    contrast: f32,    // 0-100, default 50
    gamma: f32,       // 0.3-2.8, default 1.0
    vibrance: f32,    // reserved for NVAPI
    hue: f32,         // reserved for NVAPI
}

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let hwnd = ptr::null_mut();
        let dc = GetDC(hwnd);
        if dc.is_null() {
            return Err(format!("Failed to get DC (error: {})", GetLastError()));
        }

        let gamma = settings.gamma.max(0.3).min(2.8);

        // Brightness: map 0-100 to a gamma-like adjustment
        // 50 = neutral (effective gamma multiplier = 1.0)
        // 0  = darker  (effective gamma multiplier ~1.5)
        // 100 = brighter (effective gamma multiplier ~0.6)
        let brightness_gamma = 1.0 + (50.0 - settings.brightness) / 100.0;

        // Contrast: map 0-100 to a curve steepness factor
        // 50 = neutral (factor = 1.0)
        // 0  = flat curve (factor = 0.5)
        // 100 = steep curve (factor = 1.5)
        let contrast_factor = 0.5 + (settings.contrast / 100.0);

        // Combined effective gamma
        let effective_gamma = gamma * brightness_gamma;

        let mut new_ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };

        for i in 0..256 {
            let normalized = i as f32 / 255.0;

            // Step 1: Apply combined gamma (always keeps 0->0 and 1->1)
            let gamma_val = normalized.powf(1.0 / effective_gamma);

            // Step 2: Apply contrast (S-curve around 0.5, keeps endpoints)
            let contrasted = ((gamma_val - 0.5) * contrast_factor + 0.5)
                .max(0.0)
                .min(1.0);

            // Convert to u16 (endpoints: 0->0, 255->65535)
            let val = (contrasted * 65535.0) as u16;

            // Ensure monotonically increasing
            let final_val = if i > 0 {
                val.max(new_ramp.red[i - 1])
            } else {
                val
            };

            new_ramp.red[i] = final_val;
            new_ramp.green[i] = final_val;
            new_ramp.blue[i] = final_val;
        }

        // Force endpoints to exact values (critical for driver acceptance)
        new_ramp.red[0] = 0; new_ramp.green[0] = 0; new_ramp.blue[0] = 0;
        new_ramp.red[255] = 65535; new_ramp.green[255] = 65535; new_ramp.blue[255] = 65535;

        let result = SetDeviceGammaRamp(dc, &mut new_ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        ReleaseDC(hwnd, dc);

        if result == 0 {
            return Err(format!(
                "Failed to set gamma ramp (error: {}, gamma: {:.2}, eff_gamma: {:.2}, contrast: {:.2})",
                err, gamma, effective_gamma, contrast_factor
            ));
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
