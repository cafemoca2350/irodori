use winapi::um::wingdi::{CreateDCA, DeleteDC, SetDeviceGammaRamp};
use winapi::um::libloaderapi::{LoadLibraryA, GetProcAddress, FreeLibrary};
use winapi::um::errhandlingapi::GetLastError;
use serde::Deserialize;
use tauri::{Emitter, Manager};
use std::ptr;

// ===== Gamma Ramp =====

#[repr(C)]
struct GammaRamp {
    red: [u16; 256],
    green: [u16; 256],
    blue: [u16; 256],
}

// ===== Magnification API =====

#[repr(C)]
struct MagColorEffect {
    transform: [[f32; 5]; 5],
}

type MagInitializeFn = unsafe extern "system" fn() -> i32;
type MagSetFullscreenColorEffectFn = unsafe extern "system" fn(*const MagColorEffect) -> i32;

// ===== NVAPI (Digital Vibrance) =====

type NvApiQueryInterfaceFn = unsafe extern "C" fn(id: u32) -> *const ();
type NvApiInitializeFn = unsafe extern "C" fn() -> i32;
type NvApiEnumDisplayHandleFn = unsafe extern "C" fn(index: i32, handle: *mut i32) -> i32;
// Non-Ex versions (simpler, more compatible)
type NvApiGetDVCInfoFn = unsafe extern "C" fn(handle: i32, output_id: u32, info: *mut NvDisplayDvcInfo) -> i32;
type NvApiSetDVCLevelFn = unsafe extern "C" fn(handle: i32, output_id: u32, level: i32) -> i32;

// NVAPI function IDs
const NVAPI_INITIALIZE: u32 = 0x0150E828;
const NVAPI_ENUM_DISPLAY_HANDLE: u32 = 0x9ABDD40D;
const NVAPI_GET_DVC_INFO: u32 = 0x4085DE45;
const NVAPI_SET_DVC_LEVEL: u32 = 0x172409B4;

#[repr(C)]
struct NvDisplayDvcInfo {
    version: u32,
    current_level: i32,
    min_level: i32,
    max_level: i32,
}

// MAKE_NVAPI_VERSION(NV_DISPLAY_DVC_INFO, 1) = sizeof(16) | (1 << 16)
const NV_DISPLAY_DVC_INFO_VER: u32 = 16 | (1 << 16);

// ===== Input Structs =====

#[derive(Deserialize)]
struct ColorSettings {
    brightness: f32,
    contrast: f32,
    gamma: f32,
}

#[derive(Deserialize)]
struct ColorEffect {
    saturation: f32, // 0-200, default 100
    hue: f32,        // 0-360, default 0
}

#[derive(Deserialize)]
struct VibranceSettings {
    level: i32, // 0-100, default 50
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

// ===== 1. Gamma Ramp: Brightness, Contrast, Gamma =====

#[tauri::command]
fn apply_color_settings(settings: ColorSettings) -> Result<(), String> {
    unsafe {
        let dc = get_display_dc()?;

        let gamma = settings.gamma.max(0.3).min(2.8);
        let brightness_gamma = 1.0 + (settings.brightness - 50.0) / 100.0;
        let contrast_factor = 0.5 + (settings.contrast / 100.0);
        let effective_gamma = (gamma * brightness_gamma).max(0.1).min(5.0);

        let mut ramp = GammaRamp {
            red: [0u16; 256], green: [0u16; 256], blue: [0u16; 256],
        };

        for i in 0..256 {
            let n = i as f32 / 255.0;
            let g = n.powf(1.0 / effective_gamma);
            let c = ((g - 0.5) * contrast_factor + 0.5).max(0.0).min(1.0);
            let val = (c * 65535.0) as u16;
            let final_val = if i > 0 { val.max(ramp.red[i - 1]) } else { val };
            ramp.red[i] = final_val;
            ramp.green[i] = final_val;
            ramp.blue[i] = final_val;
        }
        ramp.red[0] = 0; ramp.green[0] = 0; ramp.blue[0] = 0;
        ramp.red[255] = 65535; ramp.green[255] = 65535; ramp.blue[255] = 65535;

        let result = SetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        DeleteDC(dc);
        if result == 0 {
            return Err(format!("Failed to set gamma ramp (error: {})", err));
        }
    }
    Ok(())
}

// ===== 2. Magnification API: Hue =====

#[tauri::command]
fn apply_color_effect(effect: ColorEffect) -> Result<(), String> {
    unsafe {
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

        mag_init();

        let matrix = build_hue_matrix(effect.hue);
        let color_effect = MagColorEffect { transform: matrix };
        let result = mag_set_color(&color_effect);

        if result == 0 {
            return Err(format!("MagSetFullscreenColorEffect failed (error: {})", GetLastError()));
        }
    }
    Ok(())
}

// ===== 3. NVAPI: Digital Vibrance =====

#[tauri::command]
fn apply_vibrance(settings: VibranceSettings) -> Result<String, String> {
    unsafe {
        // Load nvapi64.dll
        let lib = LoadLibraryA(b"nvapi64.dll\0".as_ptr() as *const i8);
        if lib.is_null() {
            // Try 32-bit version
            let lib32 = LoadLibraryA(b"nvapi.dll\0".as_ptr() as *const i8);
            if lib32.is_null() {
                return Err("NVIDIA driver not found (nvapi64.dll / nvapi.dll)".to_string());
            }
            // Use 32-bit library
            return apply_vibrance_with_lib(lib32, settings.level);
        }
        apply_vibrance_with_lib(lib, settings.level)
    }
}

unsafe fn apply_vibrance_with_lib(lib: winapi::shared::minwindef::HMODULE, level: i32) -> Result<String, String> {
    let query_interface: NvApiQueryInterfaceFn = std::mem::transmute(
        GetProcAddress(lib, b"nvapi_QueryInterface\0".as_ptr() as *const i8)
    );
    if (query_interface as *const ()).is_null() {
        return Err("Failed to find nvapi_QueryInterface".to_string());
    }

    let nv_init: NvApiInitializeFn = std::mem::transmute(query_interface(NVAPI_INITIALIZE));
    let nv_enum_display: NvApiEnumDisplayHandleFn = std::mem::transmute(query_interface(NVAPI_ENUM_DISPLAY_HANDLE));
    let nv_get_dvc: NvApiGetDVCInfoFn = std::mem::transmute(query_interface(NVAPI_GET_DVC_INFO));
    let nv_set_dvc: NvApiSetDVCLevelFn = std::mem::transmute(query_interface(NVAPI_SET_DVC_LEVEL));

    let status = nv_init();
    if status != 0 {
        return Err(format!("NvAPI_Initialize failed (status: {})", status));
    }

    let mut display_handle: i32 = 0;
    let status = nv_enum_display(0, &mut display_handle);
    if status != 0 {
        return Err(format!("NvAPI_EnumNvidiaDisplayHandle failed (status: {})", status));
    }

    // Get current DVC info to know min/max range
    let mut dvc_info = NvDisplayDvcInfo {
        version: NV_DISPLAY_DVC_INFO_VER,
        current_level: 0,
        min_level: 0,
        max_level: 0,
    };
    let status = nv_get_dvc(display_handle, 0, &mut dvc_info);
    if status != 0 {
        return Err(format!("NvAPI_GetDVCInfo failed (status: {})", status));
    }

    let info = format!(
        "DVC range: {} to {} (current: {})",
        dvc_info.min_level, dvc_info.max_level, dvc_info.current_level
    );

    // Map user level (0-100) to actual DVC range
    let range = dvc_info.max_level - dvc_info.min_level;
    let target = dvc_info.min_level + (level as f32 / 100.0 * range as f32) as i32;
    let clamped = target.max(dvc_info.min_level).min(dvc_info.max_level);

    // NvAPI_SetDVCLevel takes level directly (not a struct)
    let status = nv_set_dvc(display_handle, 0, clamped);
    if status != 0 {
        return Err(format!("NvAPI_SetDVCLevel failed (status: {}). {}", status, info));
    }

    Ok(format!("Digital Vibrance set to {} ({})", clamped, info))
}

// ===== Hue rotation matrix (for Magnification API) =====

fn build_hue_matrix(hue_degrees: f32) -> [[f32; 5]; 5] {
    if hue_degrees.abs() < 0.01 {
        // Identity matrix
        return [
            [1.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0],
        ];
    }

    let theta = hue_degrees * std::f32::consts::PI / 180.0;
    let cos_a = theta.cos();
    let sin_a = theta.sin();
    let k: f32 = 1.0 / 3.0;
    let sq = k.sqrt();

    [
        [cos_a + k * (1.0 - cos_a),          k * (1.0 - cos_a) - sq * sin_a,  k * (1.0 - cos_a) + sq * sin_a, 0.0, 0.0],
        [k * (1.0 - cos_a) + sq * sin_a,     cos_a + k * (1.0 - cos_a),       k * (1.0 - cos_a) - sq * sin_a, 0.0, 0.0],
        [k * (1.0 - cos_a) - sq * sin_a,     k * (1.0 - cos_a) + sq * sin_a,  cos_a + k * (1.0 - cos_a),      0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

// ===== Diagnostic =====

#[tauri::command]
fn test_gamma() -> Result<String, String> {
    unsafe {
        let dc = get_display_dc()?;
        // Try setting identity ramp
        let mut ramp = GammaRamp { red: [0u16; 256], green: [0u16; 256], blue: [0u16; 256] };
        for i in 0..256 {
            let val = (i as u32 * 257) as u16;
            ramp.red[i] = val;
            ramp.green[i] = val;
            ramp.blue[i] = val;
        }
        let result = SetDeviceGammaRamp(dc, &mut ramp as *mut GammaRamp as *mut _);
        let err = GetLastError();
        DeleteDC(dc);
        if result == 0 {
            return Err(format!("SetDeviceGammaRamp identity test failed (error: {})", err));
        }
        Ok("Gamma ramp test PASSED".to_string())
    }
}

#[tauri::command]
fn test_nvapi() -> Result<String, String> {
    unsafe {
        let lib = LoadLibraryA(b"nvapi64.dll\0".as_ptr() as *const i8);
        if lib.is_null() {
            return Err("nvapi64.dll not found".to_string());
        }
        let qi: NvApiQueryInterfaceFn = std::mem::transmute(
            GetProcAddress(lib, b"nvapi_QueryInterface\0".as_ptr() as *const i8)
        );
        if (qi as *const ()).is_null() {
            return Err("nvapi_QueryInterface not found".to_string());
        }
        let nv_init: NvApiInitializeFn = std::mem::transmute(qi(NVAPI_INITIALIZE));
        let status = nv_init();
        if status != 0 {
            return Err(format!("NvAPI_Initialize failed (status: {})", status));
        }
        let nv_enum: NvApiEnumDisplayHandleFn = std::mem::transmute(qi(NVAPI_ENUM_DISPLAY_HANDLE));
        let mut handle: i32 = 0;
        let status = nv_enum(0, &mut handle);
        if status != 0 {
            return Err(format!("NvAPI_EnumDisplayHandle failed (status: {})", status));
        }
        let nv_get_dvc: NvApiGetDVCInfoFn = std::mem::transmute(qi(NVAPI_GET_DVC_INFO));
        let mut info = NvDisplayDvcInfo {
            version: NV_DISPLAY_DVC_INFO_VER,
            current_level: 0, min_level: 0, max_level: 0,
        };
        let status = nv_get_dvc(handle, 0, &mut info);
        if status != 0 {
            return Err(format!("NvAPI_GetDVCInfoEx failed (status: {})", status));
        }
        Ok(format!(
            "NVAPI OK! DVC: current={}, min={}, max={}",
            info.current_level, info.min_level, info.max_level
        ))
    }
}

// ===== Auto-start =====

#[tauri::command]
fn enable_autostart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("{}", e))?;
    std::process::Command::new("reg")
        .args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
               "/v", "iRodoRi", "/t", "REG_SZ", "/d", &exe.to_string_lossy(), "/f"])
        .output().map_err(|e| format!("{}", e))?;
    Ok(())
}

#[tauri::command]
fn disable_autostart() -> Result<(), String> {
    std::process::Command::new("reg")
        .args(["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
               "/v", "iRodoRi", "/f"])
        .output().map_err(|e| format!("{}", e))?;
    Ok(())
}

#[tauri::command]
fn check_autostart() -> Result<bool, String> {
    let output = std::process::Command::new("reg")
        .args(["query", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
               "/v", "iRodoRi"])
        .output().map_err(|e| format!("{}", e))?;
    Ok(output.status.success())
}

// ===== Tauri App =====

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "表示", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("iRodoRi - Display Optimizer")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => { app.exit(0); }
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
            apply_vibrance,
            test_gamma,
            test_nvapi,
            enable_autostart,
            disable_autostart,
            check_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
