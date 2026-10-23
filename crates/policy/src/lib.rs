//! Aktarım politikası. Kural: veri dışarı ancak açık sağlayıcı seçimi, dosya
//! kapsamı ve kullanıcı onayıyla çıkar. Sessiz yükseltme yoktur.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPolicy {
    Off,
    LocalOnly,
    CliAllowed,
    ApiAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    LocalModel,
    Cli,
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSensitivity {
    Normal,
    High,
    ClientConfidential,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferDecision {
    /// Cihazda işlenir; dışarı veri çıkmaz.
    LocalOnly,
    /// Politika izin veriyor ve geçerli onay var.
    Allowed,
    /// Kullanıcı onayı olmadan gönderilemez.
    RequiresConsent,
    Denied(&'static str),
}

/// Tek karar noktası. Arayüz bu fonksiyonu çağırır; kendi başına "izinli"
/// varsayımı üretemez (ADR-10).
#[must_use]
pub fn evaluate_transfer(
    policy: AiPolicy,
    destination: Destination,
    sensitivity: DataSensitivity,
    has_active_consent: bool,
) -> TransferDecision {
    if policy == AiPolicy::Off {
        return TransferDecision::Denied("E_AI_OFF");
    }
    if destination == Destination::LocalModel {
        return TransferDecision::LocalOnly;
    }
    let allowed_by_policy = match (policy, destination) {
        (AiPolicy::LocalOnly, _) => false,
        (AiPolicy::CliAllowed, Destination::Cli) => true,
        (AiPolicy::ApiAllowed, Destination::Cli | Destination::Api) => true,
        _ => false,
    };
    if !allowed_by_policy {
        return TransferDecision::Denied("E_AI_POLICY");
    }
    // Yüksek hassasiyet veya müvekkil sırrı her durumda açık onay ister.
    if sensitivity != DataSensitivity::Normal {
        return TransferDecision::RequiresConsent;
    }
    if has_active_consent {
        TransferDecision::Allowed
    } else {
        TransferDecision::RequiresConsent
    }
}

/// Onay kapsamı: sağlayıcı, dosya ve belge listesi sabitlenir; süre dolunca
/// yeniden onay gerekir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsentScope {
    pub provider_snapshot: String,
    pub case_id: String,
    pub document_ids: Vec<String>,
    pub purpose: String,
}

impl ConsentScope {
    /// Yeni sağlayıcı, yeni dosya veya listede olmayan belge kapsam dışıdır.
    #[must_use]
    pub fn covers(&self, provider_snapshot: &str, case_id: &str, document_ids: &[String]) -> bool {
        self.provider_snapshot == provider_snapshot
            && self.case_id == case_id
            && document_ids.iter().all(|d| self.document_ids.contains(d))
    }
}

/// Dosya kapsamı kontrolü: bir kayıt yalnız kendi `case_id`'si içinde görünür.
#[must_use]
pub fn same_case(requested_case: &str, row_case: &str) -> bool {
    requested_case == row_case
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_off_blocks_everything() {
        for dest in [Destination::LocalModel, Destination::Cli, Destination::Api] {
            assert_eq!(
                evaluate_transfer(AiPolicy::Off, dest, DataSensitivity::Normal, true),
                TransferDecision::Denied("E_AI_OFF")
            );
        }
    }

    #[test]
    fn local_model_never_leaves_device() {
        assert_eq!(
            evaluate_transfer(
                AiPolicy::LocalOnly,
                Destination::LocalModel,
                DataSensitivity::ClientConfidential,
                false
            ),
            TransferDecision::LocalOnly
        );
    }

    #[test]
    fn local_only_policy_blocks_cli_and_api() {
        assert_eq!(
            evaluate_transfer(
                AiPolicy::LocalOnly,
                Destination::Cli,
                DataSensitivity::Normal,
                true
            ),
            TransferDecision::Denied("E_AI_POLICY")
        );
        assert_eq!(
            evaluate_transfer(
                AiPolicy::LocalOnly,
                Destination::Api,
                DataSensitivity::Normal,
                true
            ),
            TransferDecision::Denied("E_AI_POLICY")
        );
    }

    #[test]
    fn cli_policy_does_not_enable_api() {
        assert_eq!(
            evaluate_transfer(
                AiPolicy::CliAllowed,
                Destination::Api,
                DataSensitivity::Normal,
                true
            ),
            TransferDecision::Denied("E_AI_POLICY")
        );
        assert_eq!(
            evaluate_transfer(
                AiPolicy::CliAllowed,
                Destination::Cli,
                DataSensitivity::Normal,
                false
            ),
            TransferDecision::RequiresConsent
        );
        assert_eq!(
            evaluate_transfer(
                AiPolicy::CliAllowed,
                Destination::Cli,
                DataSensitivity::Normal,
                true
            ),
            TransferDecision::Allowed
        );
    }

    #[test]
    fn client_confidential_always_requires_consent() {
        assert_eq!(
            evaluate_transfer(
                AiPolicy::ApiAllowed,
                Destination::Api,
                DataSensitivity::ClientConfidential,
                true
            ),
            TransferDecision::RequiresConsent
        );
    }

    #[test]
    fn consent_scope_is_narrow() {
        let scope = ConsentScope {
            provider_snapshot: "codex@1.2".into(),
            case_id: "case-a".into(),
            document_ids: vec!["d1".into(), "d2".into()],
            purpose: "ozet".into(),
        };
        assert!(scope.covers("codex@1.2", "case-a", &["d1".into()]));
        assert!(!scope.covers("codex@1.3", "case-a", &["d1".into()]));
        assert!(!scope.covers("codex@1.2", "case-b", &["d1".into()]));
        assert!(!scope.covers("codex@1.2", "case-a", &["d3".into()]));
    }

    #[test]
    fn cross_case_reference_is_never_allowed() {
        assert!(same_case("case-a", "case-a"));
        assert!(!same_case("case-a", "case-b"));
    }
}
