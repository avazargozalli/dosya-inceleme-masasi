//! Yerel model sağlayıcısı (Ollama uyumlu `/api/chat`).
//!
//! Belgeler cihazdan çıkmaz: uç nokta varsayılan olarak `127.0.0.1`'dir ve
//! yalnız geri döngü adresleri kabul edilir. Model yoksa çağrı denenmez;
//! `E_AI_OFF` yerine açık `E_AI_UNSUPPORTED` döner.

use std::time::Duration;

use serde_json::{json, Value};

use crate::{
    AiRequest, Candidate, Capabilities, Provider, ProviderError, ProviderProfile, ProviderType,
    ValidationState,
};

pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";

#[derive(Debug, Clone)]
pub struct LocalProvider {
    profile: ProviderProfile,
    endpoint: String,
    model: String,
    timeout: Duration,
}

impl LocalProvider {
    pub fn new(model: &str) -> Result<Self, ProviderError> {
        Self::with_endpoint(DEFAULT_ENDPOINT, model)
    }

    /// Yalnız geri döngü (`127.0.0.1`, `localhost`, `[::1]`) kabul edilir.
    pub fn with_endpoint(endpoint: &str, model: &str) -> Result<Self, ProviderError> {
        let endpoint = endpoint.trim().trim_end_matches('/');
        let allowed = endpoint.starts_with("http://127.0.0.1")
            || endpoint.starts_with("http://localhost")
            || endpoint.starts_with("http://[::1]");
        if !allowed {
            return Err(ProviderError::Unsupported("yerel model yalnız geri döngü"));
        }
        if model.trim().is_empty() {
            return Err(ProviderError::Unsupported("model adı boş"));
        }
        Ok(Self {
            profile: ProviderProfile {
                id: "local".to_string(),
                provider_type: ProviderType::LocalModel,
                adapter_id: "ollama".to_string(),
                version_range: None,
                auth_mode_verified: None,
                capabilities: Capabilities {
                    structured_output: true,
                    streaming: false,
                    tool_use: false,
                    max_context_tokens: None,
                },
                policy_version: "1".to_string(),
            },
            endpoint: endpoint.to_string(),
            model: model.trim().to_string(),
            timeout: Duration::from_secs(300),
        })
    }

    #[must_use]
    pub fn outbound_body(&self, request: &AiRequest) -> Value {
        let mut lines = String::new();
        for segment in &request.segments {
            lines.push_str(&format!("[{}] {}\n", segment.segment_id, segment.text));
        }
        json!({
            "model": self.model,
            "stream": false,
            "format": "json",
            "messages": [
                {"role": "system", "content": "Yalnız JSON döndür; uydurma, bilinmeyeni null bırak."},
                {"role": "user", "content": format!(
                    "Görev: {}\nŞema: {}\nSegmentler:\n{}",
                    request.task_type, request.output_schema_version, lines
                )},
            ],
        })
    }
}

impl Provider for LocalProvider {
    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn propose(&mut self, request: &AiRequest) -> Result<Candidate, ProviderError> {
        let url = format!("{}/api/chat", self.endpoint);
        let response = ureq::post(&url)
            .timeout(self.timeout)
            .send_json(self.outbound_body(request));
        let body: Value = match response {
            Ok(response) => response.into_json().map_err(|_| ProviderError::Schema)?,
            Err(ureq::Error::Status(404, _)) => {
                return Err(ProviderError::Unsupported("model bulunamadı"))
            }
            Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) => {
                return Err(ProviderError::Transport)
            }
        };
        let content = body
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or(ProviderError::Schema)?;
        let payload: Value = serde_json::from_str(content).map_err(|_| ProviderError::Schema)?;
        Ok(Candidate {
            payload,
            source_segment_ids: request
                .segments
                .iter()
                .map(|segment| segment.segment_id.clone())
                .collect(),
            validation_state: ValidationState::Partial,
            validation_errors: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use serial_test::serial;

    use super::*;
    use crate::ContextSegment;

    static SERVER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn request() -> AiRequest {
        AiRequest {
            case_id: "dava".into(),
            task_type: "summarize".into(),
            prompt_version: "1".into(),
            output_schema_version: "candidate.v1".into(),
            segments: vec![ContextSegment {
                segment_id: "seg-1".into(),
                text: "Kira ödenmedi.".into(),
                source_ref_json: None,
            }],
        }
    }

    fn serve(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("dinle");
        let address = listener.local_addr().expect("adres");
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[test]
    #[serial]
    fn remote_endpoint_is_rejected() {
        assert!(LocalProvider::with_endpoint("http://10.0.0.5:11434", "m").is_err());
        assert!(LocalProvider::with_endpoint("https://ollama.com", "m").is_err());
        assert!(LocalProvider::new("qwen").is_ok());
    }

    #[test]
    #[serial]
    fn parses_ollama_message_content() {
        let _guard = SERVER_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let endpoint = serve(r#"{"message":{"content":"{\"summary\":\"kira\"}"}}"#);
        let mut provider = LocalProvider::with_endpoint(&endpoint, "qwen").expect("provider");
        let candidate = provider.propose(&request()).expect("aday");
        assert_eq!(candidate.payload["summary"], "kira");
        assert_eq!(candidate.validation_state, ValidationState::Partial);
    }

    #[test]
    #[serial]
    fn body_asks_for_json_without_streaming() {
        let provider = LocalProvider::new("qwen").expect("provider");
        let body = provider.outbound_body(&request());
        assert_eq!(body["format"], "json");
        assert_eq!(body["stream"], false);
        assert!(body["messages"][1]["content"]
            .as_str()
            .unwrap_or_default()
            .contains("seg-1"));
    }
}
