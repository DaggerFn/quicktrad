use crate::config::{self, AppConfig};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex as TokioMutex;

const KNOWN_VOICES: &[(&str, &str)] = &[
    (
        "pt_BR-faber-medium",
        "https://huggingface.co/rhasspy/piper-voices/resolve/main/pt/pt_BR/faber/medium/",
    ),
    (
        "en_US-lessac-high",
        "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/high/",
    ),
];

pub struct PiperWorker {
    _child: Child,
    stdin: ChildStdin,
    stdout: TokioBufReader<ChildStdout>,
    pub _voice_name: String,
}


impl PiperWorker {
    pub async fn spawn(
        piper_bin: &Path,
        onnx_path: &Path,
        json_path: &Path,
        voice_name: &str,
        length_scale: f32,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(piper_bin);
        cmd.arg("--model").arg(onnx_path);
        cmd.arg("--config").arg(json_path);
        cmd.arg("--length_scale").arg(format!("{:.3}", length_scale));
        cmd.arg("--json-input");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Falha ao iniciar worker Piper ({}): {e}", piper_bin.display()))?;

        let stdin = child.stdin.take().ok_or("Falha ao abrir stdin do Piper")?;
        let stdout = child.stdout.take().ok_or("Falha ao abrir stdout do Piper")?;

        Ok(Self {
            _child: child,
            stdin,
            stdout: TokioBufReader::new(stdout),
            _voice_name: voice_name.to_string(),
        })
    }

    pub async fn synthesize(&mut self, text: &str, output_wav: &Path) -> Result<(), String> {
        let input_obj = serde_json::json!({
            "text": text,
            "output_file": output_wav.to_string_lossy(),
        });
        let line = format!("{}\n", input_obj);

        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("Erro ao enviar texto pro Piper: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("Erro ao enviar flush pro Piper: {e}"))?;

        let mut response_line = String::new();
        let read_res = tokio::time::timeout(
            Duration::from_secs(12),
            self.stdout.read_line(&mut response_line),
        )
        .await;

        match read_res {
            Ok(Ok(n)) if n > 0 => {
                if output_wav.exists() {
                    Ok(())
                } else {
                    Err("Piper finalizou sem gerar o arquivo de áudio".into())
                }
            }
            Ok(Ok(_)) => Err("Processo Piper encerrou a saída inesperadamente".into()),
            Ok(Err(e)) => Err(format!("Erro de leitura no Piper: {e}")),
            Err(_) => Err("Tempo esgotado aguardando síntese do áudio".into()),
        }
    }
}

use std::process::{Child as StdChild, Command as StdCommand};

struct AudioPlayer {
    current_child: Mutex<Option<StdChild>>,
    is_playing: Arc<std::sync::atomic::AtomicBool>,
}

impl AudioPlayer {
    fn new() -> Self {
        Self {
            current_child: Mutex::new(None),
            is_playing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn stop(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.current_child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    fn play(&self, wav_path: &Path, speed: f32, blocking: bool) -> Result<(), String> {
        self.stop();

        if let Some(bin) = find_system_audio_player() {
            let mut cmd = StdCommand::new(&bin);
            cmd.arg(wav_path);
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            if blocking {
                self.is_playing.store(true, Ordering::SeqCst);
                let res = cmd.status().map_err(|e| format!("Falha ao tocar via {}: {e}", bin.display()));
                self.is_playing.store(false, Ordering::SeqCst);
                return res.map(|_| ());
            } else {
                let child = cmd
                    .spawn()
                    .map_err(|e| format!("Falha ao disparar player {}: {e}", bin.display()))?;
                self.is_playing.store(true, Ordering::SeqCst);

                let is_playing_flag = self.is_playing.clone();
                if let Ok(mut guard) = self.current_child.lock() {
                    *guard = Some(child);
                }

                // Monitoramento leve para desligar a flag quando a reprodução encerrar
                std::thread::spawn(move || {
                    // Dá um tempo inicial para o player tocar
                    std::thread::sleep(Duration::from_millis(300));
                    while is_playing_flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(150));
                    }
                });
                return Ok(());
            }
        }

        // Fallback: reprodução via rodio
        let (_stream, stream_handle) = rodio::OutputStream::try_default()
            .map_err(|e| format!("Dispositivo de áudio indisponível: {e}"))?;
        let file = File::open(wav_path).map_err(|e| format!("Erro ao abrir WAV: {e}"))?;
        let source = rodio::Decoder::new(BufReader::new(file))
            .map_err(|e| format!("Erro ao decodificar áudio: {e}"))?;
        let sink = rodio::Sink::try_new(&stream_handle)
            .map_err(|e| format!("Erro ao criar sink de áudio: {e}"))?;

        sink.set_speed(speed.clamp(0.5, 2.5));
        sink.append(source);
        self.is_playing.store(true, Ordering::SeqCst);

        if blocking {
            sink.sleep_until_end();
            self.is_playing.store(false, Ordering::SeqCst);
        } else {
            let is_playing_flag = self.is_playing.clone();
            std::thread::spawn(move || {
                sink.sleep_until_end();
                is_playing_flag.store(false, Ordering::SeqCst);
            });
        }

        Ok(())
    }
}

fn find_system_audio_player() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        for name in &["pw-play", "paplay", "aplay"] {
            if let Some(p) = which_in_path(name) {
                return Some(p);
            }
        }
    }
    None
}

pub struct TtsManager {
    workers: TokioMutex<HashMap<String, PiperWorker>>,
    audio: AudioPlayer,
    counter: AtomicU64,
}

static TTS_MANAGER: OnceLock<Arc<TtsManager>> = OnceLock::new();

pub fn get_tts_manager() -> Arc<TtsManager> {
    TTS_MANAGER.get_or_init(|| Arc::new(TtsManager::new())).clone()
}

impl TtsManager {
    pub fn new() -> Self {
        Self {
            workers: TokioMutex::new(HashMap::new()),
            audio: AudioPlayer::new(),
            counter: AtomicU64::new(1),
        }
    }

    pub fn stop(&self) {
        self.audio.stop();
    }

    pub fn is_playing(&self) -> bool {
        self.audio.is_playing()
    }

    pub fn play_wav(&self, wav_path: &Path, speed: f32, blocking: bool) -> Result<(), String> {
        self.audio.play(wav_path, speed, blocking)
    }



    pub async fn speak(&self, text: &str, lang: &str, blocking: bool) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let normalized = crate::normalizer::normalize_text(trimmed, lang);
        let final_text = if normalized.is_empty() { trimmed } else { &normalized };

        let cfg = config::load();
        if !cfg.tts.enabled {
            return Err("Text-to-Speech está desativado no config.toml".into());
        }

        let voice_name = resolve_voice_for_lang(&cfg, lang);
        let piper_bin = find_piper_executable(&cfg)?;
        let voices_dir = config::voices_dir(&cfg);
        let (onnx_path, json_path) = ensure_model_files(&voice_name, &voices_dir).await?;

        let wav_id = self.counter.fetch_add(1, Ordering::Relaxed);
        let temp_wav = std::env::temp_dir().join(format!("quicktrad_tts_{}_{}.wav", std::process::id(), wav_id));

        let length_scale = (1.0 / cfg.tts.speed.clamp(0.5, 2.0)).clamp(0.5, 2.0);

        // Bloco de inferência usando o worker aquecido
        {
            let mut workers = self.workers.lock().await;
            let needs_spawn = match workers.get(&voice_name) {
                None => true,
                Some(_) => false,
            };

            if needs_spawn {
                let worker = PiperWorker::spawn(&piper_bin, &onnx_path, &json_path, &voice_name, length_scale).await?;
                workers.insert(voice_name.clone(), worker);
            }

            let worker = workers.get_mut(&voice_name).unwrap();
            if let Err(err) = worker.synthesize(final_text, &temp_wav).await {
                // Tenta reiniciar o worker uma vez se falhar (ex: processo caiu)
                workers.remove(&voice_name);
                let mut fresh_worker = PiperWorker::spawn(&piper_bin, &onnx_path, &json_path, &voice_name, length_scale).await?;
                fresh_worker
                    .synthesize(final_text, &temp_wav)
                    .await
                    .map_err(|e| format!("Falha na síntese do Piper após reinício: {e} (anterior: {err})"))?;
                workers.insert(voice_name.clone(), fresh_worker);
            }
        }


        // Toca o áudio gerado
        let play_res = self.play_wav(&temp_wav, cfg.tts.speed, blocking);


        // Limpeza assíncrona do WAV temporário após o início do playback
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let _ = tokio::fs::remove_file(temp_wav).await;
        });

        play_res
    }
}

pub fn resolve_voice_for_lang(cfg: &AppConfig, lang: &str) -> String {
    let lang_lower = lang.to_lowercase();
    let lang_prefix = lang_lower.split(['-', '_']).next().unwrap_or(&lang_lower);

    if let Some(voice) = cfg.tts.voices.get(&lang_lower) {
        return voice.clone();
    }
    if let Some(voice) = cfg.tts.voices.get(lang_prefix) {
        return voice.clone();
    }

    if lang_prefix == "pt" {
        "pt_BR-faber-medium".into()
    } else {
        "en_US-lessac-high".into()
    }
}

pub fn find_piper_executable(cfg: &AppConfig) -> Result<PathBuf, String> {
    // 1. Caminho configurado explicitamente no TOML
    if !cfg.tts.piper_path.trim().is_empty() && cfg.tts.piper_path != "piper" {
        let p = PathBuf::from(&cfg.tts.piper_path);
        if p.is_file() {
            return Ok(p);
        }
    }

    // 2. Pasta local de dados do Quicktrad (~/.local/share/quicktrad/bin/piper)
    let local_piper_dir = config::piper_bin_dir().join("piper").join("piper");
    if local_piper_dir.is_file() {
        return Ok(local_piper_dir);
    }
    let local_piper_file = config::piper_bin_dir().join("piper");
    if local_piper_file.is_file() {
        return Ok(local_piper_file);
    }

    #[cfg(windows)]
    {
        let win_dir = config::piper_bin_dir().join("piper").join("piper.exe");
        if win_dir.is_file() {
            return Ok(win_dir);
        }
        let win_file = config::piper_bin_dir().join("piper.exe");
        if win_file.is_file() {
            return Ok(win_file);
        }
    }

    // 3. Procurar no PATH do sistema operacional
    if let Some(path) = which_in_path("piper") {
        return Ok(path);
    }
    #[cfg(windows)]
    if let Some(path) = which_in_path("piper.exe") {
        return Ok(path);
    }

    Err(
        "Executável do Piper não foi encontrado. Instale o Piper no sistema ou configure o caminho no config.toml (tts.piper_path)."
            .into(),
    )
}

fn which_in_path(binary_name: &str) -> Option<PathBuf> {
    if let Ok(paths) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for part in paths.split(separator) {
            let candidate = Path::new(part).join(binary_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub async fn ensure_model_files(voice_name: &str, voices_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let onnx_path = voices_dir.join(format!("{voice_name}.onnx"));
    let json_path = voices_dir.join(format!("{voice_name}.onnx.json"));

    if onnx_path.is_file() && json_path.is_file() {
        return Ok((onnx_path, json_path));
    }

    let base_url = KNOWN_VOICES
        .iter()
        .find(|(k, _)| *k == voice_name)
        .map(|(_, url)| *url)
        .ok_or_else(|| {
            format!(
                "Arquivos de voz '{}' não foram encontrados em '{}' e a URL não é conhecida para auto-download.",
                voice_name,
                voices_dir.display()
            )
        })?;

    let onnx_url = format!("{}{}.onnx", base_url, voice_name);
    let json_url = format!("{}{}.onnx.json", base_url, voice_name);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Falha ao criar cliente HTTP: {e}"))?;

    eprintln!("[quicktrad-tts] Baixando voz '{voice_name}' do HuggingFace...");

    download_file_atomic(&client, &json_url, &json_path).await?;
    download_file_atomic(&client, &onnx_url, &onnx_path).await?;

    eprintln!("[quicktrad-tts] Voz '{voice_name}' baixada com sucesso!");
    Ok((onnx_path, json_path))
}

async fn download_file_atomic(client: &reqwest::Client, url: &str, target_path: &Path) -> Result<(), String> {
    if target_path.is_file() {
        return Ok(());
    }

    let tmp_path = target_path.with_extension("downloading");
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Erro ao contactar {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download falhou ({}) para {}", resp.status(), url));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Erro no streaming de {url}: {e}"))?;

    tokio::fs::write(&tmp_path, bytes)
        .await
        .map_err(|e| format!("Falha ao salvar temporário {}: {e}", tmp_path.display()))?;

    tokio::fs::rename(&tmp_path, target_path)
        .await
        .map_err(|e| format!("Falha ao mover arquivo para {}: {e}", target_path.display()))?;

    Ok(())
}
