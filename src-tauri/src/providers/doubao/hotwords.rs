//! Hotwords → Doubao `request.corpus.context` JSON string.
//! Meetphant `context_text` must never appear here.

use serde_json::{json, Value};

/// Build the corpus.context JSON string for Seed-ASR 2.0 submit.
/// Returns `None` when there are no hotwords (omit the field).
pub fn build_corpus_context(hotwords: &[String]) -> Option<String> {
    let words: Vec<Value> = hotwords
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .map(|w| json!({ "word": w }))
        .collect();

    if words.is_empty() {
        return None;
    }

    Some(json!({ "hotwords": words }).to_string())
}

/// Ensure a serialized body never contains Meetphant summary context under a mistaken key.
pub fn body_excludes_context_text(body: &Value) -> bool {
    !body.to_string().contains("context_text")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotwords_serializer_includes_words() {
        let ctx = build_corpus_context(&["Meetphant".into(), "豆包".into()]).expect("some");
        let parsed: Value = serde_json::from_str(&ctx).expect("json");
        assert_eq!(parsed["hotwords"][0]["word"], "Meetphant");
        assert_eq!(parsed["hotwords"][1]["word"], "豆包");
    }

    #[test]
    fn empty_hotwords_omits_context() {
        assert!(build_corpus_context(&[]).is_none());
        assert!(build_corpus_context(&["".into(), "  ".into()]).is_none());
    }

    #[test]
    fn excludes_context_text_key() {
        let ctx = build_corpus_context(&["a".into()]).unwrap();
        assert!(!ctx.contains("context_text"));
        let body = json!({
            "request": {
                "corpus": { "context": ctx }
            }
        });
        assert!(body_excludes_context_text(&body));
    }
}
