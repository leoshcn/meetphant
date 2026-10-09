//! Parse Doubao ASR JSON into speaker segments.

use serde::Deserialize;
use serde_json::Value;

use crate::models::{default_speaker_names, render_transcript_text, TranscriptSegment};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAsrTranscript {
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
    pub speaker_names: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AsrEnvelope {
    result: Option<AsrResult>,
}

#[derive(Debug, Deserialize)]
struct AsrResult {
    text: Option<String>,
    utterances: Option<Vec<AsrUtterance>>,
}

#[derive(Debug, Deserialize)]
struct AsrUtterance {
    text: Option<String>,
    additions: Option<AsrAdditions>,
    /// Some responses may put speaker at the utterance root.
    speaker: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct AsrAdditions {
    speaker: Option<Value>,
}

fn speaker_to_id(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn utterance_speaker_id(utt: &AsrUtterance) -> Option<String> {
    if let Some(additions) = &utt.additions {
        if let Some(speaker) = &additions.speaker {
            if let Some(id) = speaker_to_id(speaker) {
                return Some(id);
            }
        }
    }
    utt.speaker.as_ref().and_then(speaker_to_id)
}

/// Parse ASR `raw_json`. When utterances lack speaker ids, returns flat text only.
pub fn parse_asr_transcript(raw_json: &str, fallback_text: &str) -> ParsedAsrTranscript {
    let flat = fallback_text.to_string();
    let Ok(envelope) = serde_json::from_str::<AsrEnvelope>(raw_json) else {
        return ParsedAsrTranscript {
            text: flat,
            segments: vec![],
            speaker_names: BTreeMap::new(),
        };
    };

    let Some(result) = envelope.result else {
        return ParsedAsrTranscript {
            text: flat,
            segments: vec![],
            speaker_names: BTreeMap::new(),
        };
    };

    let result_text = result.text.clone().unwrap_or_else(|| flat.clone());
    let Some(utterances) = result.utterances else {
        return ParsedAsrTranscript {
            text: if result_text.is_empty() {
                flat
            } else {
                result_text
            },
            segments: vec![],
            speaker_names: BTreeMap::new(),
        };
    };

    let mut segments = Vec::new();
    let mut any_speaker = false;
    for utt in utterances {
        let text = utt.text.clone().unwrap_or_default();
        if text.is_empty() {
            continue;
        }
        if let Some(speaker_id) = utterance_speaker_id(&utt) {
            any_speaker = true;
            segments.push(TranscriptSegment { speaker_id, text });
        } else {
            // Keep order but without a stable speaker — treat as no diarization.
            any_speaker = false;
            break;
        }
    }

    if !any_speaker || segments.is_empty() {
        return ParsedAsrTranscript {
            text: if result_text.is_empty() {
                flat
            } else {
                result_text
            },
            segments: vec![],
            speaker_names: BTreeMap::new(),
        };
    }

    let speaker_names = default_speaker_names(&segments);
    let text = render_transcript_text(&segments, &speaker_names);
    ParsedAsrTranscript {
        text,
        segments,
        speaker_names,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_speaker_utterances() {
        let raw = r#"{
          "result": {
            "text": "你好世界",
            "utterances": [
              {"text": "你好", "additions": {"speaker": "1"}},
              {"text": "世界", "additions": {"speaker": 2}}
            ]
          }
        }"#;
        let parsed = parse_asr_transcript(raw, "fallback");
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(parsed.speaker_names.get("1").unwrap(), "发言人1");
        assert_eq!(parsed.speaker_names.get("2").unwrap(), "发言人2");
        assert!(parsed.text.contains("【发言人1】你好"));
        assert!(parsed.text.contains("【发言人2】世界"));
    }

    #[test]
    fn falls_back_without_speakers() {
        let raw = r#"{"result":{"text":"hello","utterances":[{"text":"hello"}]}}"#;
        let parsed = parse_asr_transcript(raw, "fallback");
        assert!(parsed.segments.is_empty());
        assert_eq!(parsed.text, "hello");
    }
}
