use winapi::um::wingdi::{CreateDCA, DeleteDC, GetDeviceGammaRamp, SetDeviceGammaRamp};
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

/// Get a DC that supports gamma ramp operations
unsafe fn get_display_dc() -> Result<winapi::shared::windef::HDC, String> {
    // Try \\.\DISPLAY1 first (direct display device)
    let device_name = b"\\\\.\\DISPLAY1\0";
    let dc = CreateDCA(device_name.as_ptr() as *const i8, ptr::null(), ptr::null(), ptr::null());
    if !dc.is_null() {
        return Ok(dc);
    }

    // Fallback: try "DISPLAY"
    let display = b"DISPLAY\0";
    let dc = CreateDCA(display.as_ptr() as *const i8, ptr::null(), ptr::null(), ptr::null());
    if !dc.is_null() {
        return Ok(dc);
    }

    Err(format!("Failed to get display DC (error: {})", GetLastError()))
}

/// Diagnostic: test gamma ramp API
#[tauri::command]
fn test_gamma() -> Result<String, String> {
    unsafe {
        let dc = get_display_dc()?;

        let mut ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };

        // Step 1: Read current ramp
        let get_result = GetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        if get_result == 0 {
            let err = GetLastError();
            DeleteDC(dc);
            return Err(format!("GetDeviceGammaRamp failed (error: {})", err));
        }

        let info = format!(
            "R[0]={}, R[128]={}, R[255]={}", ramp.red[0], ramp.red[128], ramp.red[255]
        );

        // Step 2: Write back unchanged
        let set_result = SetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        DeleteDC(dc);

        if set_result == 0 {
            return Err(format!("SetDeviceGammaRamp failed with unchanged ramp (error: {}). {}", err, info));
        }

        Ok(format!("PASSED! {}", info))
    }
}

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let dc = get_display_dc()?;

        let gamma = settings.gamma.max(0.3).min(2.8);
        let brightness_gamma = 1.0 + (50.0 - settings.brightness) / 100.0;
        let contrast_factor = 0.5 + (settings.contrast / 100.0);
        let effective_gamma = gamma * brightness_gamma;

        let mut new_ramp = GammaRamp {
            red: [0u16; 256],
            green: [0u16; 256],
            blue: [0u16; 256],
        };

        for i in 0..256 {
            let normalized = i as f32 / 255.0;
            let gamma_val = normalized.powf(1.0 / effective_gamma);
            let contrasted = ((gamma_val - 0.5) * contrast_factor + 0.5)
                .max(0.0)
                .min(1.0);
            let val = (contrasted * 65535.0) as u16;
            let final_val = if i > 0 { val.max(new_ramp.red[i - 1]) } else { val };

            new_ramp.red[i] = final_val;
            new_ramp.green[i] = final_val;
            new_ramp.blue[i] = final_val;
        }

        new_ramp.red[0] = 0; new_ramp.green[0] = 0; new_ramp.blue[0] = 0;
        new_ramp.red[255] = 65535; new_ramp.green[255] = 65535; new_ramp.blue[255] = 65535;

        let result = SetDeviceGammaRamp(dc, &mut new_ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        DeleteDC(dc);

        if result == 0 {
            return Err(format!("Failed to set gamma ramp (error: {})", err));
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
        .invoke_handler(tauri::generate_handler![apply_color_settings, test_gamma])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
