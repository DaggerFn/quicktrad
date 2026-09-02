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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            source_lang: default_source(),
            target_lang: default_target(),
            libretranslate_url: default_libretranslate_url(),
            api_keys: HashMap::new(),
            save_history: false,
        }
    }
}

pub fn config_dir() -> PathBuf {
    let mut dir = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    dir.push("quicktrad");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn config_path() -> PathBuf {
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
