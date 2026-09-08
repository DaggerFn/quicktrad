mod config;
mod translation;
mod usage;

use config::{AppConfig, WindowPosition};
use tauri::{
    Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Precisa rodar antes da inicialização do GTK/Tauri. Uma variável de ambiente
/// explícita sempre vence o config, permitindo override por launcher/script.
#[cfg(target_os = "linux")]
pub fn configure_linux_backend() {
    if std::env::var_os("GDK_BACKEND").is_some() {
        return;
    }

    let backend = config::load().linux_backend;
    unsafe {
        match backend {
            config::LinuxBackend::XWayland => std::env::set_var("GDK_BACKEND", "x11"),
            config::LinuxBackend::Wayland => std::env::set_var("GDK_BACKEND", "wayland"),
            config::LinuxBackend::Auto => {}
        }
    }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(selector) = app.get_webview_window("area-selector") {
        let _ = selector.show();
        let _ = selector.set_focus();
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            apply_window_config(&window, &config::load());
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        apply_window_config(&window, &config::load());
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Aplica as preferências que pertencem à janela. Essa função roda antes de
/// toda abertura, então editar o arquivo e abrir o popup novamente já basta
/// para tamanho, posição e "sempre no topo" entrarem em vigor.
fn apply_window_config(window: &tauri::WebviewWindow, cfg: &AppConfig) {
    // Limites evitam que um erro manual no TOML produza uma janela invisível
    // ou maior que um desktop comum. São pixels lógicos, portanto adaptam-se
    // corretamente a escalonamento/DPI.
    let width = cfg.window_width.clamp(320, 2_400);
    let height = cfg.window_height.clamp(180, 1_600);
    let _ = window.set_size(LogicalSize::new(width as f64, height as f64));
    let _ = window.set_always_on_top(cfg.always_on_top);

    match cfg.window_position {
        WindowPosition::CursorMonitor => move_to_cursor_monitor(window),
        WindowPosition::PrimaryMonitor => move_to_primary_monitor(window),
        WindowPosition::Fixed if !fixed_position_supported() => {
            eprintln!(
                "[quicktrad] coordenadas fixas ignoradas em Wayland nativo; use XWayland para posicionamento absoluto"
            );
        }
        WindowPosition::Fixed if fixed_position_supported() => {
            if let (Some(x), Some(y)) = (cfg.window_x, cfg.window_y) {
                let _ = window.set_position(PhysicalPosition::new(x, y));
            } else {
                // Um modo fixed sem as duas coordenadas não deve fazer o
                // popup "sumir"; usa o comportamento seguro padrão.
                move_to_cursor_monitor(window);
            }
        }
        WindowPosition::Fixed => {
            // Wayland nativo não oferece coordenadas globais controladas pelo
            // cliente. O compositor decide onde posicionar a janela.
            eprintln!("[quicktrad] window_position=fixed ignorado em Wayland nativo; use linux_backend=\"xwayland\"");
        }
    }
}

fn move_to_primary_monitor(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };

    let work_area = monitor.work_area();
    let x = work_area.position.x + (work_area.size.width.saturating_sub(size.width) / 2) as i32;
    let y = work_area.position.y + (work_area.size.height.saturating_sub(size.height) / 2) as i32;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn open_config_file(app: &tauri::AppHandle) {
    match config::prepare_for_editing() {
        Ok(path) => {
            if let Err(e) = app
                .opener()
                .open_path(path.to_string_lossy().into_owned(), None::<&str>)
            {
                eprintln!("[quicktrad] Não foi possível abrir o config.toml: {e}");
            }
        }
        Err(e) => eprintln!("[quicktrad] Não foi possível preparar o config.toml: {e}"),
    }
}

fn reload_configuration(app: &tauri::AppHandle) {
    let cfg = config::load();
    if let Some(window) = app.get_webview_window("main") {
        apply_window_config(&window, &cfg);
        let _ = window.emit("config-updated", ());
    }
}

#[cfg(target_os = "linux")]
fn fixed_position_supported() -> bool {
    !std::env::var("GDK_BACKEND")
        .unwrap_or_default()
        .split(',')
        .any(|backend| backend.trim().eq_ignore_ascii_case("wayland"))
}

#[cfg(not(target_os = "linux"))]
fn fixed_position_supported() -> bool {
    true
}

/// Abre uma janela nativa separada para escolher o retângulo. A janela
/// principal nunca muda de decoração nem recebe UI temporária, evitando os
/// estados sobrepostos que a primeira implementação produzia.
fn start_area_selection(app: &tauri::AppHandle) {
    if !fixed_position_supported() {
        eprintln!("[quicktrad] posição fixa requer Windows, X11 ou XWayland; indisponível em Wayland nativo");
        return;
    }

    if let Some(selector) = app.get_webview_window("area-selector") {
        let _ = selector.show();
        let _ = selector.set_focus();
        return;
    }

    let cfg = config::load();
    let selector = match WebviewWindowBuilder::new(
        app,
        "area-selector",
        WebviewUrl::App("area-selector.html".into()),
    )
    .title("Quicktrad — Definir área fixa")
    .inner_size(cfg.window_width as f64, cfg.window_height as f64)
    .min_inner_size(320.0, 180.0)
    .resizable(true)
    .decorations(true)
    .always_on_top(true)
    .skip_taskbar(false)
    .visible(false)
    .build()
    {
        Ok(window) => window,
        Err(error) => {
            eprintln!("[quicktrad] não foi possível abrir o seletor de área: {error}");
            return;
        }
    };

    apply_window_config(&selector, &cfg);
    let _ = selector.set_always_on_top(true);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let _ = selector.show();
    let _ = selector.set_focus();

    let app_after_close = app.clone();
    selector.on_window_event(move |event| {
        if matches!(event, WindowEvent::CloseRequested { .. }) {
            show_main_window(&app_after_close);
        }
    });
}

/// Persiste o retângulo escolhido no desktop virtual. As coordenadas são
/// físicas (necessárias para múltiplos monitores), enquanto o tamanho é salvo
/// em pixels lógicos, como as demais dimensões da janela no Tauri.
#[tauri::command]
fn save_area_selection(window: tauri::WebviewWindow) -> Result<AppConfig, String> {
    if window.label() != "area-selector" {
        return Err("comando disponível somente na janela de seleção".into());
    }
    let position = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;

    let mut cfg = config::load();
    cfg.window_position = WindowPosition::Fixed;
    cfg.window_x = Some(position.x);
    cfg.window_y = Some(position.y);
    cfg.window_width = ((size.width as f64 / scale_factor).round() as u32).clamp(320, 2_400);
    cfg.window_height = ((size.height as f64 / scale_factor).round() as u32).clamp(180, 1_600);
    config::save(&cfg)?;

    let app = window.app_handle().clone();
    window.close().map_err(|e| e.to_string())?;
    show_main_window(&app);
    Ok(cfg)
}

#[tauri::command]
fn cancel_area_selection(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "area-selector" {
        return Err("comando disponível somente na janela de seleção".into());
    }
    let app = window.app_handle().clone();
    window.close().map_err(|e| e.to_string())?;
    show_main_window(&app);
    Ok(())
}

/// Posiciona o popup no centro da área útil do monitor que contém o cursor.
/// É uma aproximação portátil do "monitor da janela ativa": normalmente o
/// cursor já está sobre ela, mas não precisamos de permissões para inspecionar
/// janelas de outros apps. Funciona em Windows e Linux/X11; se a plataforma não
/// expuser a posição global do cursor, falha silenciosamente e mantém a posição
/// atual em vez de deslocar a janela para um monitor arbitrário.
fn move_to_cursor_monitor(window: &tauri::WebviewWindow) {
    let Ok(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(Some(monitor)) = window.monitor_from_point(cursor.x, cursor.y) else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };

    let work_area = monitor.work_area();
    let x = work_area.position.x + (work_area.size.width.saturating_sub(size.width) / 2) as i32;
    let y = work_area.position.y + (work_area.size.height.saturating_sub(size.height) / 2) as i32;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Códigos de idioma aceitos como flag de linha de comando, ex:
/// `quicktrad --pt --en` (origem pt, destino en) ou `quicktrad --en` (só
/// troca o destino, mantendo a origem salva no config). Pensado para
/// diferentes binds no compositor invocarem pares diferentes.
const LANG_FLAGS: &[(&str, &str)] = &[
    ("auto", "auto"),
    ("en", "en"),
    ("pt", "pt"),
    ("es", "es"),
    ("fr", "fr"),
    ("de", "de"),
    ("du", "de"),
    ("it", "it"),
    ("ja", "ja"),
    ("jp", "ja"),
    ("zh", "zh"),
    ("ru", "ru"),
];

/// Aplica os idiomas vindos de flags de CLI ao config e avisa o frontend.
/// Retorna `true` se alguma flag de idioma foi encontrada (usado para decidir
/// se essa invocação deve **mostrar** a janela com o novo par, em vez de só
/// alternar visibilidade — presets de idioma diferentes não devem esconder
/// uma janela que já está aberta).
fn apply_lang_args(app: &tauri::AppHandle, args: &[String]) -> bool {
    let found: Vec<String> = args
        .iter()
        .filter_map(|a| a.strip_prefix("--"))
        .filter_map(|flag| LANG_FLAGS.iter().find(|(k, _)| *k == flag))
        .map(|(_, code)| code.to_string())
        .collect();

    if found.is_empty() {
        return false;
    }

    let mut cfg = config::load();
    if found.len() >= 2 {
        cfg.source_lang = found[0].clone();
        cfg.target_lang = found[1].clone();
    } else {
        cfg.target_lang = found[0].clone();
    }
    let _ = config::save(&cfg);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("config-updated", ());
    }
    true
}

#[tauri::command]
async fn translate(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let cfg = config::load();
    translation::translate_text(&cfg, &text).await
}

#[tauri::command]
fn get_config() -> AppConfig {
    config::load()
}

/// Inverte origem/destino do par atual (ex: pt→en vira en→pt). Não mexe em
/// qual idioma é qual, só troca os dois. "auto" não tem para onde inverter
/// (não sabemos que idioma foi detectado), então nesse caso retorna erro em
/// vez de adivinhar. Compartilhado pelo comando Tauri (atalho `Tab` na
/// janela) e pelo modo headless `--swap` (usado pelo plugin da barra).
fn swap_config() -> Result<AppConfig, String> {
    let mut cfg = config::load();
    if cfg.source_lang == "auto" {
        return Err("Não dá para inverter com origem \"auto\": defina um idioma de origem explícito primeiro.".into());
    }
    std::mem::swap(&mut cfg.source_lang, &mut cfg.target_lang);
    config::save(&cfg)?;
    Ok(cfg)
}

#[tauri::command]
fn swap_languages() -> Result<AppConfig, String> {
    swap_config()
}

#[tauri::command]
fn set_config(cfg: AppConfig) -> Result<(), String> {
    config::save(&cfg)
}

#[tauri::command]
fn set_font_size(font_size: u8) -> Result<u8, String> {
    config::set_font_size(font_size)
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

/// Comandos que rodam sem GUI e saem na hora — sem Tauri/GTK/webview, só um
/// runtime tokio de uma tirada. Pensado pra ser chamado como subprocesso por
/// integrações externas (ex: o plugin da barra do Omarchy) que precisam de
/// uma resposta rápida a cada tecla digitada, sem o custo de subir a janela.
/// Retorna `Some(exit_code)` se tratou um comando headless; `None` significa
/// "não é um desses, siga o fluxo normal de GUI".
pub fn try_run_headless(args: &[String]) -> Option<i32> {
    if let Some(pos) = args.iter().position(|a| a == "--query") {
        let text = args.get(pos + 1).cloned().unwrap_or_default();
        let cfg = config::load();
        let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
        return Some(rt.block_on(async {
            if text.trim().is_empty() {
                println!();
                return 0;
            }
            match translation::translate_text(&cfg, &text).await {
                Ok(t) => {
                    println!("{t}");
                    0
                }
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }));
    }

    if args.iter().any(|a| a == "--swap") {
        return Some(match swap_config() {
            Ok(cfg) => {
                println!("{} {}", cfg.source_lang, cfg.target_lang);
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }

    if args.iter().any(|a| a == "--status") {
        let cfg = config::load();
        println!("{} {}", cfg.source_lang, cfg.target_lang);
        return Some(0);
    }

    if args.iter().any(|a| a == "--usage") {
        let cfg = config::load();
        if !cfg.save_history {
            eprintln!(
                "[quicktrad] save_history está desligado no config.toml — nenhum uso foi \
                 registrado. Ligue com `save_history = true` pra passar a contar."
            );
            return Some(1);
        }
        return Some(match usage::summary(&cfg) {
            Ok(s) => {
                println!(
                    "provider={} chars_total={} entries_total={} chars_este_mes={} entries_este_mes={}",
                    s.provider, s.chars_total, s.entries, s.chars_month, s.entries_month
                );
                0
            }
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }

    None
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Precisa ser o primeiro plugin registrado. É o que permite que o atalho
    // do Hyprland (`quicktrad --toggle`) apenas acorde a instância já rodando
    // em vez de abrir um processo novo — necessário porque no Wayland (Hyprland,
    // GNOME, KDE) um app não pode registrar um hotkey global sozinho por razões
    // de segurança do protocolo; quem precisa saber da tecla é o compositor.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == "--select-area") {
                start_area_selection(app);
            } else if apply_lang_args(app, &argv) {
                show_main_window(app);
            } else {
                toggle_main_window(app);
            }
        }));
    }

    builder = builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            translate,
            get_config,
            set_config,
            set_font_size,
            swap_languages,
            save_area_selection,
            cancel_area_selection,
            hide_window
        ]);

    #[cfg(desktop)]
    {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        );
    }

    builder
        .setup(|app| {
            // Atalho global de verdade: funciona direto no Windows, macOS e
            // Linux/X11. No Windows não usamos Super+Shift+T: é o atalho
            // padrão do Text Extractor do PowerToys e pode ser remapeado para
            // a Ferramenta de Captura por outros utilitários. Em Wayland
            // (Hyprland/GNOME/KDE) o registro tende a falhar silenciosamente
            // por design da plataforma — nesse caso o usuário deve bindar a
            // tecla no compositor chamando `quicktrad --toggle` (ver README).
            let startup_args = std::env::args().collect::<Vec<_>>();
            let select_area_on_start = startup_args.iter().any(|arg| arg == "--select-area");
            apply_lang_args(&app.handle().clone(), &startup_args);

            #[cfg(desktop)]
            {
                #[cfg(target_os = "windows")]
                let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT);

                #[cfg(not(target_os = "windows"))]
                let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyT);
                if let Err(e) = app.global_shortcut().register(shortcut) {
                    eprintln!(
                        "[quicktrad] Não foi possível registrar o atalho global (esperado em Wayland/Hyprland): {e}. \
                         Configure um bind no seu compositor/DE chamando `quicktrad --toggle`."
                    );
                }
            }

            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let toggle_item = MenuItem::with_id(app, "toggle", "Mostrar/Ocultar", true, None::<&str>)?;
                let area_supported = fixed_position_supported();
                let area_label = if area_supported {
                    "Definir área fixa"
                } else {
                    "Definir área fixa (requer XWayland)"
                };
                let area_item = MenuItem::with_id(app, "select-area", area_label, area_supported, None::<&str>)?;
                let config_item = MenuItem::with_id(app, "config", "Abrir configuração", true, None::<&str>)?;
                let reload_item = MenuItem::with_id(app, "reload-config", "Recarregar configuração", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle_item, &area_item, &config_item, &reload_item, &quit_item])?;

                TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("quicktrad")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "toggle" => toggle_main_window(app),
                        "select-area" => start_area_selection(app),
                        "config" => open_config_file(app),
                        "reload-config" => reload_configuration(app),
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            toggle_main_window(tray.app_handle());
                        }
                    })
                    .build(app)?;
            }

            if let Some(window) = app.get_webview_window("main") {
                let startup_config = config::load();
                apply_window_config(&window, &startup_config);
                if startup_config.show_on_start && !select_area_on_start {
                    if let Err(e) = window.show() {
                        eprintln!("[quicktrad] show() error: {e}");
                    }
                    if let Err(e) = window.set_focus() {
                        eprintln!("[quicktrad] set_focus() error: {e}");
                    }
                }

                // A janela nasce sem foco e só recebe `Focused(true)` um instante
                // depois de aparecer; sem essa flag, esse "falso blur" inicial
                // dispararia o hide-on-blur e a escondia assim que abrisse.
                let has_focused = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let hide_target = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Focused(true) => {
                        has_focused.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    WindowEvent::Focused(false) => {
                        if has_focused.swap(false, std::sync::atomic::Ordering::SeqCst)
                            && config::load().hide_on_blur
                        {
                            let _ = hide_target.hide();
                        }
                    }
                    _ => {}
                });
            } else {
                eprintln!("[quicktrad] main window NOT FOUND");
            }

            if select_area_on_start {
                start_area_selection(&app.handle().clone());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
