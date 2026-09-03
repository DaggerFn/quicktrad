// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // GTK3/webkit2gtk usados pelo Tauri no Linux fazem chamadas específicas de
    // X11 que crasham com "Gdk-Message: Error 71 (Protocol error)" sob Wayland
    // nativo (visto no KDE Plasma 6). Forçar a GDK a rodar via XWayland evita o
    // crash; não afeta Windows/macOS nem X11 puro, onde a env var é ignorada.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() {
        unsafe {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    // webkit2gtk >= 2.42 usa por padrão um renderer via DMA-BUF/GBM pra
    // aceleração de GPU; sob XWayland (ver fix acima) essa alocação falha
    // silenciosamente em várias combinações de driver/compositor (KDE
    // Plasma incluso) e a webview fica em branco/preta sem erro fatal.
    // Desligar volta pro renderer via software, que sempre funciona.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let args: Vec<String> = std::env::args().collect();
    if let Some(exit_code) = quicktrad_lib::try_run_headless(&args) {
        std::process::exit(exit_code);
    }
    quicktrad_lib::run()
}
