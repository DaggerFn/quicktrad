use crate::config::AppConfig;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

/// Todo novo motor de tradução implementa este trait e é plugado em `build_provider`.
#[async_trait]
trait Provider: Send + Sync {
    async fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String>;
}

struct LibreTranslate {
    url: String,
    api_key: Option<String>,
}

#[async_trait]
impl Provider for LibreTranslate {
    async fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let mut body = json!({
            "q": text,
            "source": source,
            "target": target,
            "format": "text",
        });
        if let Some(key) = &self.api_key {
            body["api_key"] = json!(key);
        }

        let res = client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Falha ao contactar LibreTranslate: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("LibreTranslate retornou {status}: {text}"));
        }

        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "translatedText")]
            translated_text: String,
        }
        let parsed: Resp = res
            .json()
            .await
            .map_err(|e| format!("Resposta inesperada do LibreTranslate: {e}"))?;
        Ok(parsed.translated_text)
    }
}

/// Endpoint público não-oficial do Google Translate. Sem API key, mas sujeito a
/// rate-limit/instabilidade — bom fallback, não recomendado para uso pesado.
struct GoogleFree;

#[async_trait]
impl Provider for GoogleFree {
    async fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let res = client
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", source),
                ("tl", target),
                ("dt", "t"),
                ("q", text),
            ])
            .send()
            .await
            .map_err(|e| format!("Falha ao contactar Google Translate: {e}"))?;

        let value: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Resposta inesperada do Google Translate: {e}"))?;

        let sentences = value
            .get(0)
            .and_then(|v| v.as_array())
            .ok_or("Resposta inesperada do Google Translate")?;

        let mut out = String::new();
        for s in sentences {
            if let Some(chunk) = s.get(0).and_then(|v| v.as_str()) {
                out.push_str(chunk);
            }
        }
        Ok(out)
    }
}

/// TM (translation memory) gratuita, sem API key — bom default "funciona de
/// primeira", mas não tem auto-detecção real de idioma nem qualidade de MT
/// consistente para frases fora do comum. Para uso sério, trocar para
/// "deepl" ou um LibreTranslate self-hosted no config.toml.
struct MyMemory;

#[async_trait]
impl Provider for MyMemory {
    async fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        if source == "auto" {
            return Err(
                "O provedor MyMemory não faz auto-detecção de idioma: escolha o idioma de origem manualmente."
                    .into(),
            );
        }
        let client = reqwest::Client::new();
        let res = client
            .get("https://api.mymemory.translated.net/get")
            .query(&[("q", text), ("langpair", &format!("{source}|{target}"))])
            .send()
            .await
            .map_err(|e| format!("Falha ao contactar MyMemory: {e}"))?;

        #[derive(Deserialize)]
        struct ResponseData {
            #[serde(rename = "translatedText")]
            translated_text: String,
        }
        #[derive(Deserialize)]
        struct Match {
            translation: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "responseData")]
            response_data: ResponseData,
            #[serde(default)]
            matches: Vec<Match>,
        }
        let parsed: Resp = res
            .json()
            .await
            .map_err(|e| format!("Resposta inesperada do MyMemory: {e}"))?;

        // O match top-1 às vezes vem com translatedText vazio (entrada de TM
        // sem tradução preenchida); nesse caso usa o primeiro match não-vazio
        // da lista em vez de devolver uma tradução em branco.
        if !parsed.response_data.translated_text.trim().is_empty() {
            return Ok(parsed.response_data.translated_text);
        }
        parsed
            .matches
            .into_iter()
            .map(|m| m.translation)
            .find(|t| !t.trim().is_empty())
            .ok_or_else(|| "MyMemory não encontrou nenhuma tradução para esse texto.".into())
    }
}

/// Requer `api_keys.deepl` no config. Chaves free terminam em ":fx" e usam o
/// host api-free.deepl.com; chaves pro usam api.deepl.com.
struct DeepL {
    api_key: String,
    pro: bool,
}

#[async_trait]
impl Provider for DeepL {
    async fn translate(&self, text: &str, source: &str, target: &str) -> Result<String, String> {
        let client = reqwest::Client::new();
        let base = if self.pro {
            "https://api.deepl.com/v2/translate"
        } else {
            "https://api-free.deepl.com/v2/translate"
        };

        let mut params = vec![
            ("auth_key", self.api_key.as_str()),
            ("text", text),
            ("target_lang", target),
        ];
        if source != "auto" {
            params.push(("source_lang", source));
        }

        let res = client
            .post(base)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Falha ao contactar DeepL: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("DeepL retornou {status}: {body}"));
        }

        #[derive(Deserialize)]
        struct Trans {
            text: String,
        }
        #[derive(Deserialize)]
        struct Resp {
            translations: Vec<Trans>,
        }
        let parsed: Resp = res
            .json()
            .await
            .map_err(|e| format!("Resposta inesperada do DeepL: {e}"))?;
        parsed
            .translations
            .into_iter()
            .next()
            .map(|t| t.text)
            .ok_or_else(|| "DeepL não retornou tradução".into())
    }
}

/// Placeholder para provedores ainda não implementados (ex: Argos Translate
/// offline, que exige um binário/python local — plugar aqui quando quiser).
struct Unimplemented(String);

#[async_trait]
impl Provider for Unimplemented {
    async fn translate(&self, _text: &str, _source: &str, _target: &str) -> Result<String, String> {
        Err(format!("Provedor '{}' ainda não implementado.", self.0))
    }
}

fn build_provider(cfg: &AppConfig) -> Box<dyn Provider> {
    match cfg.provider.as_str() {
        "mymemory" => Box::new(MyMemory),
        "libretranslate" => Box::new(LibreTranslate {
            url: cfg.libretranslate_url.clone(),
            api_key: cfg.api_keys.get("libretranslate").cloned(),
        }),
        "google" => Box::new(GoogleFree),
        "deepl" => match cfg.api_keys.get("deepl") {
            Some(key) => Box::new(DeepL {
                api_key: key.clone(),
                pro: !key.ends_with(":fx"),
            }),
            None => Box::new(Unimplemented("deepl (defina api_keys.deepl no config.toml)".into())),
        },
        other => Box::new(Unimplemented(other.to_string())),
    }
}

pub async fn translate_text(cfg: &AppConfig, text: &str) -> Result<String, String> {
    build_provider(cfg)
        .translate(text, &cfg.source_lang, &cfg.target_lang)
        .await
}
