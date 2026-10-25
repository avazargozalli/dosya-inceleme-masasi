//! OpenAI-uyumlu API sağlayıcısı. **Ağ çağrısı yapar**; bu yüzden yalnız
//! avukat onayı + aktif aktarım onayı varsa çağrılır (`crates/storage::ai`).
//!
//! Anahtar **hiçbir yere yazılmaz**: yalnız `DIM_API_KEY` ortam değişkeninden
//! okunur ve istek başlığında kullanılır (INV-11). Sağlayıcı yalnız **aday**
//! üretir; hukuki kayıt avukat onayıyla oluşur (INV-08).

use std::time::Duration;

use serde_json::{json, Value};

use crate::{
    AiRequest, Candidate, Capabilities, Provider, ProviderError, ProviderProfile, ProviderType,
    ValidationState,
};

const SYSTEM_PROMPT: &str = "Yalnız JSON döndür. Hukuki sonuç üretme; yalnız aday alanlar öner. \
Verilmeyen bilgiyi uydurma; bilinmeyeni null bırak.";

#[derive(Debug, Clone)]
pub struct ApiProvider {
    profile: ProviderProfile,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    timeout: Duration,
}

impl ApiProvider {
    /// Uç nokta `https://` olmalıdır; düz HTTP reddedilir.
    pub fn new(endpoint: &str, model: &str) -> Result<Self, ProviderError> {
        let endpoint = endpoint.trim().trim_end_matches('/');
        if !endpoint.starts_with("https://") {
            return Err(ProviderError::Unsupported("uç nokta https olmalı"));
        }
        if model.trim().is_empty() {
            return Err(ProviderError::Unsupported("model adı boş"));
        }
        Ok(Self::build(endpoint, model))
    }

    fn build(endpoint: &str, model: &str) -> Self {
        Self {
            profile: ProviderProfile {
                id: "api".to_string(),
                provider_type: ProviderType::Api,
                adapter_id: "openai-compatible".to_string(),
                version_range: None,
                auth_mode_verified: Some("env:DIM_API_KEY".to_string()),
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
            api_key: std::env::var("DIM_API_KEY").ok(),
            timeout: Duration::from_secs(120),
        }
    }

    #[cfg(test)]
    pub fn for_test(endpoint: &str, model: &str) -> Self {
        Self::build(endpoint, model)
    }

    #[must_use]
    pub fn outbound_body(&self, request: &AiRequest) -> Value {
        let mut lines = String::new();
        for segment in &request.segments {
            lines.push_str(&format!("[{}] {}\n", segment.segment_id, segment.text));
        }
        json!({
            "model": self.model,
            "temperature": 0,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": format!(
                    "Görev: {}\nŞema: {}\nSegmentler:\n{}",
                    request.task_type, request.output_schema_version, lines
                )},
            ],
        })
    }
}

impl Provider for ApiProvider {
    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn propose(&mut self, request: &AiRequest) -> Result<Candidate, ProviderError> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or(ProviderError::Unsupported("DIM_API_KEY tanımlı değil"))?;
        let url = format!("{}/chat/completions", self.endpoint);
        let response = ureq::post(&url)
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("Content-Type", "application/json")
            .timeout(self.timeout)
            .send_json(self.outbound_body(request));

        let body: Value = match response {
            Ok(response) => response.into_json().map_err(|_| ProviderError::Schema)?,
            Err(ureq::Error::Status(429, _)) => return Err(ProviderError::Quota),
            Err(ureq::Error::Status(401 | 403, _)) => {
                return Err(ProviderError::Unsupported("kimlik reddedildi"))
            }
            Err(ureq::Error::Status(_, _)) | Err(ureq::Error::Transport(_)) => {
                return Err(ProviderError::Transport)
            }
        };

        let content = body
            .pointer("/choices/0/message/content")
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
            // Çekirdek alıntıları ayrıca doğrular; sağlayıcı kendi kendini
            // doğrulanmış ilan edemez.
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

    /// Testler paralel çalışıyor; gerçek soket trafiğini sıraya al.
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

    /// İsteği **tamamen** okur (başlıklar + `Content-Length` kadar gövde).
    ///
    /// Tek `read` yeterli değildi: istemci gövdeyi iki TCP parçasında
    /// gönderirse sunucu başlıkları okuyup hemen yanıtlayıp soketi kapatıyordu;
    /// kapanan soket istemcinin kalan yazımına RST ile karşılık verip **yanıtı
    /// düşürebiliyordu** (macOS CI'da `Transport`, ubuntu'da geçiyordu).
    fn read_request(stream: &mut std::net::TcpStream) {
        use std::time::Duration;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut received: Vec<u8> = Vec::new();
        let mut buffer = [0u8; 4096];
        let header_end = loop {
            match stream.read(&mut buffer) {
                Ok(0) => return,
                Ok(n) => {
                    received.extend_from_slice(&buffer[..n]);
                    if let Some(pos) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                    if received.len() > 64 * 1024 {
                        return;
                    }
                }
                Err(_) => return,
            }
        };

        // `Content-Length` (varsa) kadar gövdeyi de oku; kalan bayt bırakma.
        let headers = String::from_utf8_lossy(&received[..header_end]).to_ascii_lowercase();
        let declared: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        let mut remaining = declared.saturating_sub(received.len() - header_end);
        while remaining > 0 {
            match stream.read(&mut buffer) {
                Ok(0) => return,
                Ok(n) => remaining = remaining.saturating_sub(n),
                Err(_) => return,
            }
        }
    }

    /// Gerçek bir TCP sunucusu: ureq gerçek HTTP konuşur. `Content-Length`
    /// gövdeden hesaplanır — elle yazılan uzunluk testi bozuyordu.
    fn serve(status: &str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("dinle");
        let address = listener.local_addr().expect("adres");
        let status = status.to_string();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                read_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                // Yazma yönünü kapat, okuma yönünü boşalt: istemci yanıtı
                // aldıktan sonra kapanır, biz de EOF görürüz. Soketi hemen
                // düşürmek RST üretip yanıtı kaybettirebiliyordu.
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let mut sink = [0u8; 1024];
                while let Ok(n) = stream.read(&mut sink) {
                    if n == 0 {
                        break;
                    }
                }
            }
        });
        format!("http://{address}")
    }

    #[test]
    #[serial]
    fn https_is_required() {
        assert!(ApiProvider::new("http://ornek.com", "m").is_err());
        assert!(ApiProvider::new("https://ornek.com", "m").is_ok());
    }

    #[test]
    #[serial]
    fn outbound_body_carries_segments_and_schema() {
        let provider = ApiProvider::new("https://ornek.com/v1", "m").expect("provider");
        let body = provider.outbound_body(&request());
        assert_eq!(body["model"], "m");
        let user = body["messages"][1]["content"].as_str().unwrap_or_default();
        assert!(user.contains("seg-1"), "{user}");
        assert!(user.contains("candidate.v1"), "{user}");
    }

    #[test]
    #[serial]
    fn parses_choice_content_as_candidate() {
        let _guard = SERVER_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let endpoint = serve(
            "200 OK",
            r#"{"choices":[{"message":{"content":"{\"summary\":\"kira\"}"}}]}"#,
        );
        let mut provider = ApiProvider::for_test(&endpoint, "m");
        // Test ortamında anahtar yok; varlığını taklit etmek için doğrudan ata.
        provider.api_key = Some("test-anahtar".into());
        let candidate = provider.propose(&request()).expect("aday");
        assert_eq!(candidate.payload["summary"], "kira");
        assert_eq!(candidate.source_segment_ids, vec!["seg-1".to_string()]);
        assert_eq!(candidate.validation_state, ValidationState::Partial);
    }

    #[test]
    #[serial]
    fn missing_key_is_reported_not_sent() {
        let mut provider = ApiProvider::new("https://ornek.com/v1", "m").expect("provider");
        provider.api_key = None;
        let error = provider.propose(&request()).unwrap_err();
        assert_eq!(error.code(), "E_AI_UNSUPPORTED");
    }

    #[test]
    #[serial]
    fn quota_and_schema_are_distinguished() {
        let _guard = SERVER_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let endpoint = serve("429 Too Many Requests", "");
        let mut provider = ApiProvider::for_test(&endpoint, "m");
        provider.api_key = Some("k".into());
        assert_eq!(
            provider.propose(&request()).unwrap_err(),
            ProviderError::Quota
        );

        drop(_guard);
        let _guard = SERVER_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let endpoint = serve("200 OK", r#"{"choices":[{"message":{}}]}"#);
        let mut provider = ApiProvider::for_test(&endpoint, "m");
        provider.api_key = Some("k".into());
        assert_eq!(
            provider.propose(&request()).unwrap_err(),
            ProviderError::Schema
        );
    }
}
