use winapi::um::wingdi::{CreateDCA, DeleteDC, GetDeviceGammaRamp, SetDeviceGammaRamp};
use winapi::um::libloaderapi::{LoadLibraryA, GetProcAddress};
use winapi::um::errhandlingapi::GetLastError;
use serde::Deserialize;
use tauri::{Emitter, Manager};
use std::ptr;

// ===== Gamma Ramp (brightness, contrast, gamma) =====

#[repr(C)]
struct GammaRamp {
    red: [u16; 256],
    green: [u16; 256],
    blue: [u16; 256],
}

// ===== Magnification API (saturation, hue) =====

#[repr(C)]
struct MagColorEffect {
    transform: [[f32; 5]; 5],
}

type MagInitializeFn = unsafe extern "system" fn() -> i32;
type MagSetFullscreenColorEffectFn = unsafe extern "system" fn(*const MagColorEffect) -> i32;

// ===== Structs =====

#[derive(Deserialize)]
struct ColorSettings {
    brightness: f32,  // 0-100, default 50
    contrast: f32,    // 0-100, default 50
    gamma: f32,       // 0.3-2.8, default 1.0
}

#[derive(Deserialize)]
struct ColorEffect {
    saturation: f32,  // 0-200, default 100 (100=neutral)
    hue: f32,         // 0-360, default 0
}

// ===== Display DC helper =====

unsafe fn get_display_dc() -> Result<winapi::shared::windef::HDC, String> {
    let device_name = b"\\\\.\\DISPLAY1\0";
    let dc = CreateDCA(device_name.as_ptr() as *const i8, ptr::null(), ptr::null(), ptr::null());
    if !dc.is_null() {
        return Ok(dc);
    }
    let display = b"DISPLAY\0";
    let dc = CreateDCA(display.as_ptr() as *const i8, ptr::null(), ptr::null(), ptr::null());
    if !dc.is_null() {
        return Ok(dc);
    }
    Err(format!("Failed to get display DC (error: {})", GetLastError()))
}

// ===== Gamma Ramp command =====

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let dc = get_display_dc()?;

        let gamma = settings.gamma.max(0.3).min(2.8);
        let brightness_gamma = 1.0 + (settings.brightness - 50.0) / 100.0;
        let contrast_factor = 0.5 + (settings.contrast / 100.0);
        let effective_gamma = (gamma * brightness_gamma).max(0.1).min(5.0);

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

// ===== Magnification API command =====

#[tauri::command]
fn apply_color_effect(effect: ColorEffect) -> Result<(), String> {
    unsafe {
        // Load Magnification.dll dynamically
        let lib = LoadLibraryA(b"Magnification.dll\0".as_ptr() as *const i8);
        if lib.is_null() {
            return Err(format!("Failed to load Magnification.dll (error: {})", GetLastError()));
        }

        let mag_init: MagInitializeFn = std::mem::transmute(
            GetProcAddress(lib, b"MagInitialize\0".as_ptr() as *const i8)
        );
        let mag_set_color: MagSetFullscreenColorEffectFn = std::mem::transmute(
            GetProcAddress(lib, b"MagSetFullscreenColorEffect\0".as_ptr() as *const i8)
        );

        // Initialize magnification
        mag_init();

        // Build the combined color transformation matrix
        let matrix = build_color_matrix(effect.saturation, effect.hue);
        let color_effect = MagColorEffect { transform: matrix };

        let result = mag_set_color(&color_effect);
        // Do NOT call MagUninitialize - the effect must persist

        if result == 0 {
            return Err(format!("MagSetFullscreenColorEffect failed (error: {})", GetLastError()));
        }
    }
    Ok(())
}

/// Build a 5x5 color transformation matrix combining saturation and hue rotation
fn build_color_matrix(saturation_pct: f32, hue_degrees: f32) -> [[f32; 5]; 5] {
    // Saturation: map 0-200 to factor 0.0-2.0 (100 = 1.0 = neutral)
    let s = saturation_pct / 100.0;

    // Luminance weights (ITU-R BT.709)
    let lr: f32 = 0.2126;
    let lg: f32 = 0.7152;
    let lb: f32 = 0.0722;

    // Saturation matrix
    let sat = [
        [lr * (1.0 - s) + s, lg * (1.0 - s),     lb * (1.0 - s),     0.0, 0.0],
        [lr * (1.0 - s),     lg * (1.0 - s) + s,  lb * (1.0 - s),     0.0, 0.0],
        [lr * (1.0 - s),     lg * (1.0 - s),       lb * (1.0 - s) + s, 0.0, 0.0],
        [0.0,                0.0,                   0.0,                 1.0, 0.0],
        [0.0,                0.0,                   0.0,                 0.0, 1.0],
    ];

    if hue_degrees.abs() < 0.01 {
        // No hue rotation needed
        return sat;
    }

    // Hue rotation matrix (rotation around the (1,1,1) axis in RGB space)
    let theta = hue_degrees * std::f32::consts::PI / 180.0;
    let cos_a = theta.cos();
    let sin_a = theta.sin();
    let k: f32 = 1.0 / 3.0;
    let sq = (k as f32).sqrt(); // sqrt(1/3)

    let hue = [
        [cos_a + k * (1.0 - cos_a),          k * (1.0 - cos_a) - sq * sin_a,  k * (1.0 - cos_a) + sq * sin_a, 0.0, 0.0],
        [k * (1.0 - cos_a) + sq * sin_a,     cos_a + k * (1.0 - cos_a),       k * (1.0 - cos_a) - sq * sin_a, 0.0, 0.0],
        [k * (1.0 - cos_a) - sq * sin_a,     k * (1.0 - cos_a) + sq * sin_a,  cos_a + k * (1.0 - cos_a),      0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ];

    // Multiply: result = saturation * hue
    multiply_5x5(&sat, &hue)
}

fn multiply_5x5(a: &[[f32; 5]; 5], b: &[[f32; 5]; 5]) -> [[f32; 5]; 5] {
    let mut result = [[0.0f32; 5]; 5];
    for i in 0..5 {
        for j in 0..5 {
            for k in 0..5 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

// ===== Diagnostic =====

#[tauri::command]
fn test_gamma() -> Result<String, String> {
    unsafe {
        let dc = get_display_dc()?;
        let mut ramp = GammaRamp { red: [0u16; 256], green: [0u16; 256], blue: [0u16; 256] };
        let get_result = GetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        if get_result == 0 {
            let err = GetLastError();
            DeleteDC(dc);
            return Err(format!("GetDeviceGammaRamp failed (error: {})", err));
        }
        let info = format!("R[0]={}, R[128]={}, R[255]={}", ramp.red[0], ramp.red[128], ramp.red[255]);
        let set_result = SetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        DeleteDC(dc);
        if set_result == 0 {
            return Err(format!("SetDeviceGammaRamp failed (error: {}). {}", err, info));
        }
        Ok(format!("Gamma PASSED! {}", info))
    }
}

// ===== Tauri App =====

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
        .invoke_handler(tauri::generate_handler![
            apply_color_settings,
            apply_color_effect,
            test_gamma
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
