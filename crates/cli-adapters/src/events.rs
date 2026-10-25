//! Codex `exec --json` olay taşıması.
//!
//! Codex, NDJSON satırları üretir: `thread.started`, `turn.started`,
//! `item.completed` (ajan mesajı), `turn.completed` (kullanım). Ürün bu olayları
//! **ayrıştırır**; ham metni "cevap" sanmaz. Bilinmeyen olay tipi sessizce
//! yutulmaz, `unknown` olarak kayda geçer; bozuk satır hata döndürür.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexEvent {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CodexUsage>,
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("bozuk JSON satırı: {0}")]
    BadLine(String),
    #[error("olay nesnesinde `type` yok")]
    MissingType,
}

fn number(value: Option<&serde_json::Value>) -> u64 {
    value.and_then(serde_json::Value::as_u64).unwrap_or(0)
}

/// NDJSON çıktısını olaylara çevirir. Boş satırlar atlanır.
pub fn parse_codex_json_lines(output: &str) -> Result<Vec<CodexEvent>, EventError> {
    let mut events = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|_| EventError::BadLine(line.chars().take(80).collect()))?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(EventError::MissingType)?
            .to_string();
        let text = value
            .pointer("/item/text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let usage = value.get("usage").map(|usage| CodexUsage {
            input_tokens: number(usage.get("input_tokens")),
            cached_input_tokens: number(usage.get("cached_input_tokens")),
            output_tokens: number(usage.get("output_tokens")),
            reasoning_output_tokens: number(usage.get("reasoning_output_tokens")),
        });
        events.push(CodexEvent { kind, text, usage });
    }
    Ok(events)
}

/// Son ajan mesajı: ürünün göstereceği metin. Yoksa `None` — uydurulmaz.
#[must_use]
pub fn final_message(events: &[CodexEvent]) -> Option<&str> {
    events
        .iter()
        .rev()
        .find(|event| event.kind == "item.completed" && event.text.is_some())
        .and_then(|event| event.text.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = r#"{"type":"thread.started","thread_id":"01a174b9"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"DIM-CANARY-47"}}
{"type":"turn.completed","usage":{"input_tokens":27390,"cached_input_tokens":12288,"cache_write_input_tokens":0,"output_tokens":10,"reasoning_output_tokens":0}}"#;

    #[test]
    fn parses_real_codex_stream() {
        let events = parse_codex_json_lines(REAL).expect("parse");
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].kind, "thread.started");
        assert_eq!(final_message(&events), Some("DIM-CANARY-47"));
        let usage = events[3].usage.as_ref().expect("usage");
        assert_eq!(usage.input_tokens, 27390);
        assert_eq!(usage.output_tokens, 10);
    }

    #[test]
    fn unknown_event_is_kept_not_swallowed() {
        let events = parse_codex_json_lines(r#"{"type":"future.event","x":1}"#).expect("parse");
        assert_eq!(events[0].kind, "future.event");
        assert!(events[0].text.is_none());
    }

    #[test]
    fn broken_line_is_an_error() {
        assert!(matches!(
            parse_codex_json_lines("{bozuk"),
            Err(EventError::BadLine(_))
        ));
    }

    #[test]
    fn missing_type_is_reported() {
        assert!(matches!(
            parse_codex_json_lines(r#"{"text":"x"}"#),
            Err(EventError::MissingType)
        ));
    }

    #[test]
    fn no_message_means_none_not_empty_string() {
        let events = parse_codex_json_lines(r#"{"type":"turn.completed"}"#).expect("parse");
        assert_eq!(final_message(&events), None);
    }
}
