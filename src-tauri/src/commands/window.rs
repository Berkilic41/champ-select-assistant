use tauri::{Emitter, Window};

#[tauri::command]
pub async fn set_always_on_top(window: Window, enabled: bool) -> Result<(), String> {
    window.set_always_on_top(enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_window_size(window: Window, preset: String) -> Result<(), String> {
    let (w, h) = match preset.as_str() {
        "compact" => (320u32, 480u32),
        "wide" => (1100u32, 700u32),
        // Slim horizontal banner pinned over the LCU during champ-select.
        "overlay" => (560u32, 180u32),
        _ => (800u32, 600u32), // "standard" default
    };
    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: w,
            height: h,
        }))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hide_window(window: Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn show_window(window: Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_window_opacity(window: Window, opacity: f64) -> Result<(), String> {
    // Tauri 2'de opacity plugin ile veya doğrudan Windows API ile yapılır
    // Basit yaklaşım: webview'ın arka planını saydam yap + window layer ile
    // Windows'ta SetLayeredWindowAttributes gerektiriyor — bu sprint için
    // CSS opacity çözümü daha pratik
    // Sadece event emit et, frontend CSS ile uygulasın
    window
        .emit("opacity-changed", opacity)
        .map_err(|e| e.to_string())
}
