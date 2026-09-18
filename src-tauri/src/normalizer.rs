use crate::config;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::sync::{OnceLock, RwLock};

const DEFAULT_REPLACEMENTS_TOML: &str = include_str!("../replacements.toml");

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReplacementsConfig {
    #[serde(default)]
    pub pt: HashMap<String, String>,
    #[serde(default)]
    pub en: HashMap<String, String>,
}

pub struct TextNormalizer {
    pt_regex: Option<Regex>,
    pt_map: HashMap<String, String>,
    en_regex: Option<Regex>,
    en_map: HashMap<String, String>,
}

impl TextNormalizer {
    pub fn load() -> Self {
        // 1. Carregar valores padrão embutidos
        let mut config: ReplacementsConfig =
            toml::from_str(DEFAULT_REPLACEMENTS_TOML).unwrap_or_default();

        // 2. Se o usuário tiver um replacements.toml em seu diretório de configuração,
        // mesclar as substituições personalizadas por cima dos defaults
        let user_path = config::config_dir().join("replacements.toml");
        if user_path.is_file() {
            if let Ok(content) = fs::read_to_string(&user_path) {
                if let Ok(user_cfg) = toml::from_str::<ReplacementsConfig>(&content) {
                    for (k, v) in user_cfg.pt {
                        config.pt.insert(k, v);
                    }
                    for (k, v) in user_cfg.en {
                        config.en.insert(k, v);
                    }
                }
            }
        }

        let (pt_regex, pt_map) = Self::compile_lang_map(config.pt);
        let (en_regex, en_map) = Self::compile_lang_map(config.en);

        Self {
            pt_regex,
            pt_map,
            en_regex,
            en_map,
        }
    }

    fn compile_lang_map(raw_map: HashMap<String, String>) -> (Option<Regex>, HashMap<String, String>) {
        if raw_map.is_empty() {
            return (None, HashMap::new());
        }

        let mut lookup_map = HashMap::with_capacity(raw_map.len());
        for (k, v) in raw_map {
            lookup_map.insert(k.to_lowercase(), v);
        }

        let mut keys: Vec<&String> = lookup_map.keys().collect();
        // Ordenar chaves das mais longas para as mais curtas para dar prioridade a compostos
        // como "w/o" antes de "w/", "tl;dr" antes de "dps", etc.
        keys.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));

        let mut patterns = Vec::with_capacity(keys.len());
        for k in keys {
            let starts_word = k.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false);
            let ends_word = k.chars().last().map(|c| c.is_alphanumeric()).unwrap_or(false);
            let escaped = regex::escape(k);

            let pat = match (starts_word, ends_word) {
                (true, true) => format!(r"\b{}\b", escaped),
                (true, false) => format!(r"\b{}", escaped),
                (false, true) => format!(r"{}\b", escaped),
                (false, false) => escaped,
            };
            patterns.push(pat);
        }

        let combined = format!("(?i)(?:{})", patterns.join("|"));
        let regex = match Regex::new(&combined) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("[quicktrad-normalizer] Falha ao compilar regex de substituições: {e}");
                None
            }
        };

        (regex, lookup_map)
    }

    pub fn normalize(&self, text: &str, lang: &str) -> String {
        let lang_lower = lang.to_lowercase();
        let lang_prefix = lang_lower.split(['-', '_']).next().unwrap_or(&lang_lower);

        let (re, map) = if lang_prefix == "pt" {
            (self.pt_regex.as_ref(), &self.pt_map)
        } else {
            (self.en_regex.as_ref(), &self.en_map)
        };

        let Some(re) = re else {
            return text.to_string();
        };

        let replaced = re.replace_all(text, |caps: &regex::Captures| {
            let matched = &caps[0];
            let lower = matched.to_lowercase();
            if let Some(replacement) = map.get(&lower) {
                replacement.clone()
            } else {
                matched.to_string()
            }
        });

        // Colapsa múltiplos espaços em branco consecutivos que possam ter surgido
        let mut cleaned = String::with_capacity(replaced.len());
        let mut last_was_space = false;
        for ch in replaced.chars() {
            if ch.is_whitespace() {
                if !last_was_space {
                    cleaned.push(' ');
                    last_was_space = true;
                }
            } else {
                cleaned.push(ch);
                last_was_space = false;
            }
        }
        cleaned.trim().to_string()
    }
}

static NORMALIZER: OnceLock<RwLock<TextNormalizer>> = OnceLock::new();

pub fn get_normalizer() -> &'static RwLock<TextNormalizer> {
    NORMALIZER.get_or_init(|| RwLock::new(TextNormalizer::load()))
}

pub fn normalize_text(text: &str, lang: &str) -> String {
    if let Ok(guard) = get_normalizer().read() {
        guard.normalize(text, lang)
    } else {
        text.to_string()
    }
}

pub fn reload_normalizer() {
    if let Ok(mut guard) = get_normalizer().write() {
        *guard = TextNormalizer::load();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pt_normalization() {
        let normalizer = TextNormalizer::load();
        let input = "olá vc, td bem? vou c/ vc hoje / sla";
        let output = normalizer.normalize(input, "pt");
        assert_eq!(output, "olá você, tudo bem? vou com você hoje barra sei lá");
    }

    #[test]
    fn test_word_boundaries_no_false_positives() {
        let normalizer = TextNormalizer::load();
        let input = "o advogado banana quer ajuda";
        let output = normalizer.normalize(input, "pt");
        // "vc" inside "advogado" or "n" inside "banana" or "q" inside "quer" must not trigger
        assert_eq!(output, "o advogado banana quer ajuda");
    }

    #[test]
    fn test_en_normalization() {
        let normalizer = TextNormalizer::load();
        let input = "idk wtf you mean w/ this / tbh";
        let output = normalizer.normalize(input, "en");
        assert_eq!(output, "I don't know what the fuck you mean with this slash to be honest");
    }

    #[test]
    fn test_case_insensitivity() {
        let normalizer = TextNormalizer::load();
        let input = "IDK WTF VC TÁ FALANDO";
        let output_en = normalizer.normalize(input, "en");
        assert!(output_en.contains("I don't know what the fuck"));
        let output_pt = normalizer.normalize(input, "pt");
        assert!(output_pt.contains("você está"));
    }
}
