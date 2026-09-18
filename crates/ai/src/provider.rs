//! Proveedores LLM: reglas (offline), Ollama y OpenAI-compatible.

use super::AiConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiMode {
    Rules,
    Local,
    Cloud,
}

impl AiMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rules => "Solo reglas (offline)",
            Self::Local => "Modelo local (Ollama/llama.cpp)",
            Self::Cloud => "Nube (OpenAI-compatible)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

pub trait LlmBackend: Send {
    fn complete(&self, messages: &[ChatMessage], config: &AiConfig) -> anyhow::Result<String>;
}

/// Backend determinista: responde con frases coherentes según palabras clave.
/// Es el fallback cuando no hay red/modelo o el jugador lo elige.
pub struct RulesBackend;

impl LlmBackend for RulesBackend {
    fn complete(&self, messages: &[ChatMessage], _config: &AiConfig) -> anyhow::Result<String> {
        let last = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.to_lowercase())
            .unwrap_or_default();
        let reply = if last.contains("hola") || last.contains("saludo") {
            "Saludos, caminante. El viento trae historias nuevas hoy."
        } else if last.contains("portal") || last.contains("dimensión") || last.contains("dimension") {
            "Los portales no son puertas: son espejos. Solo reflejan lo que ya llevas dentro."
        } else if last.contains("karma") || last.contains("alma") {
            "Tu balanza tiene tres fieles: compasión, justicia y sabiduría. No olvides ninguno."
        } else if last.contains("ayuda") || last.contains("misión") || last.contains("mision") {
            "Mira el diario. Lo que buscas suele estar donde dejaste de mirar."
        } else if last.contains("infierno") {
            "El Infierno no castiga: devuelve. Siembra bien y recogerás luz."
        } else if last.contains("cielo") {
            "El Cielo abre para quien puede quedarse quieto y escuchar."
        } else if last.contains("pulso") {
            "La brasa estable es la raíz del Pulso. Aprende a guiarla y las máquinas antiguas te oirán."
        } else if last.contains("quién") || last.contains("quien") {
            "Soy un eco de los que se fueron, pero mi memoria sigue aquí para ti."
        } else {
            "Interesante... cuéntame más. Los sabios aprenden escuchando."
        };
        Ok(reply.to_string())
    }
}

pub struct OllamaBackend;

impl LlmBackend for OllamaBackend {
    fn complete(&self, messages: &[ChatMessage], config: &AiConfig) -> anyhow::Result<String> {
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
            .collect();
        let body = serde_json::json!({
            "model": config.model,
            "messages": msgs,
            "stream": false,
            "options": {
                "temperature": config.temperature,
                "num_predict": config.max_tokens,
            }
        });
        let url = format!("{}/api/chat", config.url.trim_end_matches('/'));
        let resp = ureq::post(&url)
            .config()
            .timeout_global(Some(std::time::Duration::from_millis(config.timeout_ms)))
            .build()
            .send_json(&body)?;
        let v: serde_json::Value = resp.into_body().read_json()?;
        let text = v
            .pointer("/message/content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if text.is_empty() {
            anyhow::bail!("respuesta vacía de Ollama");
        }
        Ok(text)
    }
}

pub struct OpenAiCompatBackend;

impl LlmBackend for OpenAiCompatBackend {
    fn complete(&self, messages: &[ChatMessage], config: &AiConfig) -> anyhow::Result<String> {
        let api_key = std::env::var(&config.api_key_env).unwrap_or_default();
        if api_key.is_empty() {
            // Fallback a local style: llama.cpp server no requiere clave.
            tracing::warn!("{} no definida; se intenta sin clave", config.api_key_env);
        }
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
            .collect();
        let body = serde_json::json!({
            "model": config.model,
            "messages": msgs,
            "max_tokens": config.max_tokens,
            "temperature": config.temperature,
            "stream": false,
        });
        let base = config.url.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        };
        let mut req = ureq::post(&url);
        if !api_key.is_empty() {
            req = req.header("Authorization", &format!("Bearer {api_key}"));
        }
        let resp = req
            .config()
            .timeout_global(Some(std::time::Duration::from_millis(config.timeout_ms)))
            .build()
            .send_json(&body)?;
        let v: serde_json::Value = resp.into_body().read_json()?;
        let text = v
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if text.is_empty() {
            anyhow::bail!("respuesta vacía del proveedor");
        }
        Ok(text)
    }
}
