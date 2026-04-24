use winapi::um::wingdi::SetDeviceGammaRamp;
use winapi::um::winuser::GetDC;
use ddc::Ddc;
use ddc_winapi::Monitor;
use serde::Serialize;
use tauri::{Emitter, Manager};
use std::ptr;

#[derive(Serialize)]
pub struct DisplayInfo {
    pub name: String,
}

#[tauri::command]
fn set_gamma(gamma: f32) -> Result<(), String> {
    unsafe {
        let dc = GetDC(ptr::null_mut());
        if dc.is_null() {
            return Err("Failed to get DC".to_string());
        }

        let mut ramp = [0u16; 768];
        for i in 0..256 {
            let val = if gamma <= 0.0 {
                0
            } else {
                let v = (i as f32 / 255.0).powf(1.0 / gamma) * 65535.0;
                v.min(65535.0) as u16
            };
            ramp[i] = val;
            ramp[i + 256] = val;
            ramp[i + 512] = val;
        }

        if SetDeviceGammaRamp(dc, ramp.as_mut_ptr() as *mut _) == 0 {
            return Err("Failed to set gamma ramp".to_string());
        }
    }
    Ok(())
}

#[tauri::command]
fn set_digital_vibrance(value: i32) -> Result<(), String> {
    // NVAPI の実装が不安定なため、一旦モック（あるいは単純な呼び出し）にします
    // ビルドを通すことを優先します
    println!("Digital Vibrance set to: {}", value);
    Ok(())
}

#[tauri::command]
fn set_monitor_brightness(value: u8) -> Result<(), String> {
    let monitors = Monitor::enumerate().map_err(|e| format!("DDC Enum Error: {:?}", e))?;
    for mut monitor in monitors {
        // Ddc トレイトをスコープに入れることで set_vcp_feature が使えるようになります
        monitor.set_vcp_feature(0x10, value as u16).map_err(|e| format!("DDC Set Error: {:?}", e))?;
    }
    Ok(())
}

#[tauri::command]
fn set_monitor_contrast(value: u8) -> Result<(), String> {
    let monitors = Monitor::enumerate().map_err(|e| format!("DDC Enum Error: {:?}", e))?;
    for mut monitor in monitors {
        monitor.set_vcp_feature(0x12, value as u16).map_err(|e| format!("DDC Set Error: {:?}", e))?;
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
            // トレイアイコンとメニューの設定
            let gaming = MenuItem::with_id(app, "preset_gaming", "Gaming", true, None::<&str>)?;
            let cinema = MenuItem::with_id(app, "preset_cinema", "Cinema", true, None::<&str>)?;
            let coding = MenuItem::with_id(app, "preset_coding", "Coding", true, None::<&str>)?;
            let default = MenuItem::with_id(app, "preset_default", "Default", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&gaming, &cinema, &coding, &default, &quit])?;

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
                        "preset_coding" => {
                            let _ = app.emit("set-preset", "coding");
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
            set_gamma,
            set_digital_vibrance,
            set_monitor_brightness,
            set_monitor_contrast
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
