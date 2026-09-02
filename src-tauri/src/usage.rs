use crate::config::AppConfig;
use rusqlite::Connection;
use std::time::{SystemTime, UNIX_EPOCH};

/// Histórico local de uso, opt-in via `save_history` no config. Grava cada
/// texto digitado num sqlite (`usage.db`, mesma pasta do config.toml) pra dar
/// pra contar quantos caracteres (unidade de cobrança da DeepL, não
/// "tokens") já foram gastos com o provider atual — útil pra acompanhar o
/// limite do tier free (500k caracteres/mês). Fica de fora do binário
/// principal (`config.rs`/`translation.rs`) pra manter esse acoplamento
/// opcional isolado num arquivo só.
fn db_path() -> std::path::PathBuf {
    let mut dir = crate::config::config_dir();
    dir.push("usage.db");
    dir
}

fn open() -> Result<Connection, String> {
    let conn = Connection::open(db_path()).map_err(|e| format!("Falha ao abrir usage.db: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS translations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            provider TEXT NOT NULL,
            source_lang TEXT NOT NULL,
            target_lang TEXT NOT NULL,
            chars INTEGER NOT NULL,
            text TEXT NOT NULL
        )",
        (),
    )
    .map_err(|e| format!("Falha ao criar tabela em usage.db: {e}"))?;
    Ok(conn)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Grava uma entrada de uso, se `save_history` estiver ligado no config.
/// Chamado a cada tradução, com o texto de origem (o que foi de fato enviado
/// pro provider e conta pra cobrança) — não o resultado traduzido.
pub fn log_if_enabled(cfg: &AppConfig, text: &str) {
    if !cfg.save_history || text.trim().is_empty() {
        return;
    }
    let chars = text.chars().count() as i64;
    let result = (|| -> Result<(), String> {
        let conn = open()?;
        conn.execute(
            "INSERT INTO translations (ts, provider, source_lang, target_lang, chars, text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (now_unix(), &cfg.provider, &cfg.source_lang, &cfg.target_lang, chars, text),
        )
        .map_err(|e| format!("Falha ao gravar em usage.db: {e}"))?;
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("[quicktrad] {e}");
    }
}

pub struct UsageSummary {
    pub provider: String,
    pub entries: i64,
    pub chars_total: i64,
    pub entries_month: i64,
    pub chars_month: i64,
}

/// Resumo de uso pro provider atual: total histórico e total desde o
/// início do mês corrente (pensado pra acompanhar limites mensais tipo o
/// tier free da DeepL, 500k caracteres/mês).
pub fn summary(cfg: &AppConfig) -> Result<UsageSummary, String> {
    let conn = open()?;

    let month_start = {
        let secs_per_day = 86_400i64;
        let today_start = (now_unix() / secs_per_day) * secs_per_day;
        // Aproximação: volta até o dia 1 do mês corrente contando dias, sem
        // depender de crate de calendário — suficiente pro caso de uso (uma
        // estimativa de "desde o início do mês", não um relatório fiscal).
        let mut t = today_start;
        loop {
            let day_of_month = day_of_month_utc(t);
            if day_of_month == 1 {
                break;
            }
            t -= secs_per_day;
        }
        t
    };

    let (entries, chars_total): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(chars), 0) FROM translations WHERE provider = ?1",
            [&cfg.provider],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Falha ao ler usage.db: {e}"))?;

    let (entries_month, chars_month): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(chars), 0) FROM translations WHERE provider = ?1 AND ts >= ?2",
            (&cfg.provider, month_start),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Falha ao ler usage.db: {e}"))?;

    Ok(UsageSummary {
        provider: cfg.provider.clone(),
        entries,
        chars_total,
        entries_month,
        chars_month,
    })
}

/// Dia do mês (UTC) de um timestamp unix, calculado à mão (sem crate de
/// calendário) via o algoritmo civil_from_days de Howard Hinnant.
fn day_of_month_utc(unix_secs: i64) -> u32 {
    let z = unix_secs.div_euclid(86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    d as u32
}
