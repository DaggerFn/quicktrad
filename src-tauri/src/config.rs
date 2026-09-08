use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_source")]
    pub source_lang: String,
    #[serde(default = "default_target")]
    pub target_lang: String,
    #[serde(default = "default_libretranslate_url")]
    pub libretranslate_url: String,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    /// Opt-in: grava cada texto digitado (e sua contagem de caracteres) num
    /// banco sqlite local, pra acompanhar quanto de uso (caracteres, unidade
    /// de cobrança da DeepL) já foi gasto. Desligado por padrão porque grava
    /// o texto literal digitado, não só o total — ver README.
    #[serde(default)]
    pub save_history: bool,
    /// Tamanho da fonte das áreas de texto da janela flutuante. Preferência
    /// ajustada por atalho, sem acrescentar controles à UI minimalista.
    #[serde(default = "default_font_size")]
    pub font_size: u8,
    /// Largura da janela em pixels lógicos. Mantemos isso no arquivo, em vez
    /// de no `tauri.conf.json`, para cada usuário poder adaptar ao monitor.
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    /// Altura da janela em pixels lógicos.
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Onde centralizar o popup a cada abertura. `cursor_monitor` é o padrão
    /// porque acompanha naturalmente o monitor em que a pessoa trabalha.
    #[serde(default)]
    pub window_position: WindowPosition,
    /// Coordenada X física no desktop virtual, usada somente por
    /// `window_position = "fixed"`.
    #[serde(default)]
    pub window_x: Option<i32>,
    /// Coordenada Y física no desktop virtual, usada somente por
    /// `window_position = "fixed"`.
    #[serde(default)]
    pub window_y: Option<i32>,
    /// Mantém o popup acima das outras janelas enquanto ele está visível.
    #[serde(default = "default_always_on_top")]
    pub always_on_top: bool,
    /// Esconde o popup ao perder foco, sem encerrar o processo da bandeja.
    #[serde(default = "default_hide_on_blur")]
    pub hide_on_blur: bool,
    /// Mostra a janela já no início do processo. `false` inicia apenas na
    /// bandeja e espera o atalho global, o menu ou `quicktrad --toggle`.
    #[serde(default = "default_show_on_start")]
    pub show_on_start: bool,
    /// Backend gráfico usado no Linux. XWayland permite posicionamento
    /// absoluto; Wayland nativo deixa a posição sob controle do compositor.
    /// Não tem efeito em Windows/macOS e pode ser sobrescrito por GDK_BACKEND.
    #[serde(default)]
    pub linux_backend: LinuxBackend,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowPosition {
    #[default]
    CursorMonitor,
    PrimaryMonitor,
    Fixed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum LinuxBackend {
    #[default]
    #[serde(rename = "xwayland")]
    XWayland,
    #[serde(rename = "wayland")]
    Wayland,
    #[serde(rename = "auto")]
    Auto,
}

fn default_provider() -> String {
    // Suporte oficial hoje: DeepL, pela qualidade (motor neural de verdade,
    // não translation-memory como o MyMemory). Exige api_keys.deepl — sem
    // isso a tradução retorna erro pedindo pra configurar. Tier grátis da
    // DeepL (deepl.com/en/pro-api) não pede cartão. Quem preferir zero
    // configuração pode trocar pra "mymemory" no config.toml (sem key, mas
    // qualidade bem mais instável — ver README). Arquitetura de providers
    // vai ficar mais plugável (issue rastreando isso no repo).
    "deepl".into()
}

fn default_source() -> String {
    // "pt" (em vez de "auto") de propósito: o atalho de swap (Tab) não
    // sabe pra onde inverter com origem "auto", então esse seria um erro
    // logo na primeira tecla pra quem não mexeu na config ainda.
    "pt".into()
}

fn default_target() -> String {
    "en".into()
}

fn default_libretranslate_url() -> String {
    "https://libretranslate.com/translate".into()
}

fn default_font_size() -> u8 {
    14
}

fn default_window_width() -> u32 {
    520
}

fn default_window_height() -> u32 {
    240
}

fn default_always_on_top() -> bool {
    true
}

fn default_hide_on_blur() -> bool {
    true
}

fn default_show_on_start() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            source_lang: default_source(),
            target_lang: default_target(),
            libretranslate_url: default_libretranslate_url(),
            api_keys: HashMap::new(),
            save_history: false,
            font_size: default_font_size(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            window_position: WindowPosition::default(),
            window_x: None,
            window_y: None,
            always_on_top: default_always_on_top(),
            hide_on_blur: default_hide_on_blur(),
            show_on_start: default_show_on_start(),
            linux_backend: LinuxBackend::default(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    dir.push("quicktrad");
    let _ = fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    let mut dir = config_dir();
    dir.push("config.toml");
    dir
}

pub fn load() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => {
            let cfg = AppConfig::default();
            let _ = save(&cfg);
            cfg
        }
    }
}

pub fn save(cfg: &AppConfig) -> Result<(), String> {
    let path = config_path();
    let contents = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, contents).map_err(|e| e.to_string())
}

/// Garante que o arquivo exista e o regrava com o esquema atual. É chamado
/// apenas ao abrir pelo menu, assim novos campos aparecem para instalações
/// antigas sem regravar o TOML a cada tradução.
pub fn prepare_for_editing() -> Result<PathBuf, String> {
    let path = config_path();
    let cfg = load();
    save(&cfg)?;
    Ok(path)
}

pub fn set_font_size(font_size: u8) -> Result<u8, String> {
    // Limites conservadores: 12 px continua legível em telas densas e 28 px
    // ainda preserva o layout compacto do popup.
    let font_size = font_size.clamp(12, 28);
    let mut cfg = load();
    cfg.font_size = font_size;
    save(&cfg)?;
    Ok(font_size)
}
