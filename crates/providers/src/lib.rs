//! Sağlayıcı sözleşmesi. Yerel model, resmî CLI ve doğrudan API aynı arayüzü
//! uygular; kaynak doğrulaması sağlayıcının dışındadır (ADR-04).
//!
//! Bu crate ağ veya süreç çağrısı yapmaz. Sağlayıcılar `propose` ile yalnız
//! **aday** üretir; aday kalıcı hukuk kaydına dönüşmez (INV-08).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Off,
    LocalModel,
    Cli,
    Api,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub structured_output: bool,
    pub streaming: bool,
    pub tool_use: bool,
    pub max_context_tokens: Option<u32>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self { structured_output: false, streaming: false, tool_use: false, max_context_tokens: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub provider_type: ProviderType,
    pub adapter_id: String,
    pub version_range: Option<String>,
    /// Doğrulanmış kimlik türü. OAuth token'ı veya çerez **tutulmaz** (INV-11).
    pub auth_mode_verified: Option<String>,
    pub capabilities: Capabilities,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSegment {
    pub segment_id: String,
    pub text: String,
    /// Kaynak referansı JSON'u sağlayıcıya gönderilmez; yalnız yerel eşleme için.
    pub source_ref_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRequest {
    pub case_id: String,
    pub task_type: String,
    pub prompt_version: String,
    pub output_schema_version: String,
    pub segments: Vec<ContextSegment>,
}

impl AiRequest {
    /// Gönderilecek toplam bayt; aktarım kaydı ve kapsam hesabı için kullanılır.
    #[must_use]
    pub fn outbound_bytes(&self) -> usize {
        self.segments.iter().map(|s| s.text.len()).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    Valid,
    Invalid,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub payload: Value,
    /// Adayın dayandığı segment kimlikleri; çekirdek alıntıyı ayrıca doğrular.
    pub source_segment_ids: Vec<String>,
    pub validation_state: ValidationState,
    pub validation_errors: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("AI kapalı; istek gönderilmedi")]
    Disabled,
    #[error("sağlayıcı bu işi desteklemiyor: {0}")]
    Unsupported(&'static str),
    #[error("sağlayıcı bağlantısı kurulamadı")]
    Transport,
    #[error("sağlayıcı kotası doldu")]
    Quota,
    #[error("yanıt beklenen şemaya uymuyor")]
    Schema,
    #[error("işlem iptal edildi")]
    Cancelled,
}

impl ProviderError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Disabled => "E_AI_OFF",
            Self::Unsupported(_) => "E_AI_UNSUPPORTED",
            Self::Transport => "E_AI_TRANSPORT",
            Self::Quota => "E_AI_QUOTA",
            Self::Schema => "E_AI_SCHEMA",
            Self::Cancelled => "E_AI_CANCELLED",
        }
    }

    /// Kota/ağ hataları yeniden denenebilir; şema ve kapalı durum değil.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        matches!(self, Self::Transport | Self::Quota)
    }
}

pub trait Provider {
    fn profile(&self) -> &ProviderProfile;
    fn propose(&mut self, request: &AiRequest) -> Result<Candidate, ProviderError>;
}

/// AI kapalı profil. Hiçbir isteği kabul etmez; temel işlevler AI olmadan
/// çalışmaya devam eder (REQ-015).
#[derive(Debug, Clone)]
pub struct OffProvider {
    profile: ProviderProfile,
}

impl OffProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: ProviderProfile {
                id: "off".to_string(),
                provider_type: ProviderType::Off,
                adapter_id: "off".to_string(),
                version_range: None,
                auth_mode_verified: None,
                capabilities: Capabilities::default(),
                policy_version: "1".to_string(),
            },
        }
    }
}

impl Default for OffProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for OffProvider {
    fn profile(&self) -> &ProviderProfile {
        &self.profile
    }

    fn propose(&mut self, _request: &AiRequest) -> Result<Candidate, ProviderError> {
        Err(ProviderError::Disabled)
    }
}

/// Sağlayıcı yetenek kapısı: istek, profilini aşan bir şey istiyorsa gönderilmez.
pub fn ensure_capability(profile: &ProviderProfile, request: &AiRequest, require_structured: bool) -> Result<(), ProviderError> {
    if profile.provider_type == ProviderType::Off {
        return Err(ProviderError::Disabled);
    }
    if require_structured && !profile.capabilities.structured_output {
        return Err(ProviderError::Unsupported("structured_output"));
    }
    if let Some(limit) = profile.capabilities.max_context_tokens {
        // Kaba tahmin: 4 bayt ≈ 1 token. Kesin sayım sağlayıcıya bırakılmaz.
        let approx_tokens = u32::try_from(request.outbound_bytes() / 4).unwrap_or(u32::MAX);
        if approx_tokens > limit {
            return Err(ProviderError::Unsupported("context_limit"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AiRequest {
        AiRequest {
            case_id: "case-1".into(),
            task_type: "summarize".into(),
            prompt_version: "p1".into(),
            output_schema_version: "1".into(),
            segments: vec![ContextSegment { segment_id: "s1".into(), text: "kira".into(), source_ref_json: None }],
        }
    }

    #[test]
    fn off_provider_refuses_everything() {
        let mut p = OffProvider::new();
        assert_eq!(p.propose(&request()).unwrap_err(), ProviderError::Disabled);
        assert_eq!(p.profile().provider_type, ProviderType::Off);
    }

    #[test]
    fn capability_gate_blocks_missing_structured_output() {
        let profile = ProviderProfile {
            id: "p".into(),
            provider_type: ProviderType::Cli,
            adapter_id: "claude".into(),
            version_range: None,
            auth_mode_verified: Some("subscription".into()),
            capabilities: Capabilities::default(),
            policy_version: "1".into(),
        };
        assert_eq!(
            ensure_capability(&profile, &request(), true).unwrap_err(),
            ProviderError::Unsupported("structured_output")
        );
        assert!(ensure_capability(&profile, &request(), false).is_ok());
    }

    #[test]
    fn context_limit_is_enforced_conservatively() {
        let profile = ProviderProfile {
            id: "p".into(),
            provider_type: ProviderType::Api,
            adapter_id: "api".into(),
            version_range: None,
            auth_mode_verified: None,
            capabilities: Capabilities { structured_output: true, streaming: false, tool_use: false, max_context_tokens: Some(1) },
            policy_version: "1".into(),
        };
        let mut req = request();
        req.segments[0].text = "x".repeat(100);
        assert_eq!(ensure_capability(&profile, &req, true).unwrap_err(), ProviderError::Unsupported("context_limit"));
    }

    #[test]
    fn outbound_bytes_counts_only_text() {
        assert_eq!(request().outbound_bytes(), 4);
    }

    #[test]
    fn errors_have_stable_codes() {
        assert_eq!(ProviderError::Quota.code(), "E_AI_QUOTA");
        assert!(ProviderError::Quota.retryable());
        assert!(!ProviderError::Schema.retryable());
    }
}
