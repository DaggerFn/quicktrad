// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // O padrão continua sendo XWayland pelos bugs GTK/WebKit já documentados,
    // mas o usuário pode escolher Wayland nativo ou seleção automática em
    // config.toml. GDK_BACKEND explícito tem precedência sobre o arquivo.
    #[cfg(target_os = "linux")]
    quicktrad_lib::configure_linux_backend();

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
