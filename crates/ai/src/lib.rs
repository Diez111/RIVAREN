//! `rivaren-ai`: diálogo de NPCs con proveedores híbridos.
//!
//! Modos:
//! - `Rules`: determinista, offline, siempre disponible (fallback).
//! - `Local`: Ollama / llama.cpp-server en localhost (OpenAI-compatible).
//! - `Cloud`: API OpenAI-compatible remota (opt-in, clave por variable de entorno).
//!
//! Nunca bloquea el frame: un worker en hilo dedicado atiende la cola y
//! devuelve tokens por canal; la UI consume eventos.

pub mod prompt;
pub mod provider;

pub use prompt::{NpcContext, build_messages, build_system_prompt};
pub use provider::{AiMode, ChatMessage, LlmBackend, RulesBackend, OllamaBackend, OpenAiCompatBackend};

use serde::{Deserialize, Serialize};
use std::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub mode: AiMode,
    pub url: String,
    pub model: String,
    pub api_key_env: String,
    pub timeout_ms: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            mode: AiMode::Rules,
            url: "http://localhost:11434".into(),
            model: "qwen3:4b".into(),
            api_key_env: "RIVAREN_AI_KEY".into(),
            timeout_ms: 3000,
            max_tokens: 180,
            temperature: 0.8,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AiEvent {
    Token { id: u64, text: String },
    Done { id: u64, full: String },
    Error { id: u64, message: String },
}

struct Job {
    id: u64,
    messages: Vec<ChatMessage>,
    config: AiConfig,
}

/// Servicio de IA con worker dedicado (sin bloquear el hilo de juego).
pub struct AiService {
    tx: mpsc::Sender<Job>,
    rx: mpsc::Receiver<AiEvent>,
    next_id: u64,
    pub config: AiConfig,
}

impl AiService {
    pub fn new(config: AiConfig) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (ev_tx, ev_rx) = mpsc::channel::<AiEvent>();
        std::thread::Builder::new()
            .name("rivaren-ai".into())
            .spawn(move || worker(job_rx, ev_tx))
            .expect("hilo de IA");
        Self {
            tx: job_tx,
            rx: ev_rx,
            next_id: 1,
            config,
        }
    }

    pub fn ask(&mut self, messages: Vec<ChatMessage>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let job = Job {
            id,
            messages,
            config: self.config.clone(),
        };
        let _ = self.tx.send(job);
        id
    }

    /// Eventos disponibles este frame (no bloquea).
    pub fn poll(&self) -> Vec<AiEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            out.push(ev);
        }
        out
    }
}

fn worker(jobs: mpsc::Receiver<Job>, events: mpsc::Sender<AiEvent>) {
    while let Ok(job) = jobs.recv() {
        let backend = build_backend(&job.config);
        match backend.complete(&job.messages, &job.config) {
            Ok(text) => {
                // Emula streaming por chunks para una UI fluida.
                let mut acc = String::new();
                let chars: Vec<char> = text.chars().collect();
                for chunk in chars.chunks(6) {
                    let piece: String = chunk.iter().collect();
                    acc.push_str(&piece);
                    if events
                        .send(AiEvent::Token {
                            id: job.id,
                            text: piece,
                        })
                        .is_err()
                    {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(12));
                }
                let _ = events.send(AiEvent::Done {
                    id: job.id,
                    full: acc,
                });
            }
            Err(e) => {
                let _ = events.send(AiEvent::Error {
                    id: job.id,
                    message: e.to_string(),
                });
            }
        }
    }
}

fn build_backend(config: &AiConfig) -> Box<dyn LlmBackend> {
    match config.mode {
        AiMode::Rules => Box::new(RulesBackend),
        AiMode::Local | AiMode::Cloud => {
            if is_openai_style(&config.url) {
                Box::new(OpenAiCompatBackend)
            } else {
                Box::new(OllamaBackend)
            }
        }
    }
}

fn is_openai_style(url: &str) -> bool {
    url.contains("/v1") || url.contains("openai") || url.contains("chat/completions")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_backend_answers() {
        let b = RulesBackend;
        let cfg = AiConfig::default();
        let out = b
            .complete(&[ChatMessage::user("¿Quién eres?")], &cfg)
            .unwrap();
        assert!(!out.is_empty());
    }
    #[test]
    fn service_async_roundtrip() {
        let mut svc = AiService::new(AiConfig::default());
        let id = svc.ask(vec![ChatMessage::user("hola")]);
        let mut got_done = false;
        for _ in 0..400 {
            for ev in svc.poll() {
                if let AiEvent::Done { id: did, full } = ev {
                    assert_eq!(did, id);
                    assert!(!full.is_empty());
                    got_done = true;
                }
            }
            if got_done {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(got_done, "sin respuesta de IA en 4s");
    }
}
