mod config;
mod translation;

use config::AppConfig;
use tauri::{Emitter, Manager, WindowEvent};

#[cfg(desktop)]
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
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
            if apply_lang_args(app, &argv) {
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
            swap_languages,
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
            // Linux/X11. Em Wayland (Hyprland/GNOME/KDE) o registro tende a
            // falhar silenciosamente por design da plataforma — nesse caso o
            // usuário deve bindar a tecla no compositor chamando
            // `quicktrad --toggle` (ver README).
            apply_lang_args(&app.handle().clone(), &std::env::args().collect::<Vec<_>>());

            #[cfg(desktop)]
            {
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
                let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle_item, &quit_item])?;

                TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("quicktrad")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "toggle" => toggle_main_window(app),
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
                if let Err(e) = window.show() {
                    eprintln!("[quicktrad] show() error: {e}");
                }
                if let Err(e) = window.set_focus() {
                    eprintln!("[quicktrad] set_focus() error: {e}");
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
                        if has_focused.swap(false, std::sync::atomic::Ordering::SeqCst) {
                            let _ = hide_target.hide();
                        }
                    }
                    _ => {}
                });
            } else {
                eprintln!("[quicktrad] main window NOT FOUND");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
