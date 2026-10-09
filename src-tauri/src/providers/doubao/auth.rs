//! Doubao speech new-console auth: a single `X-Api-Key` header.
//! The old-console `X-Api-App-Key` + `X-Api-Access-Key` pair is not supported.

use reqwest::blocking::RequestBuilder;

use crate::services::credentials::DoubaoCredentials;

pub const API_KEY_HEADER: &str = "X-Api-Key";

/// Attach Doubao auth headers to a request (submit, query, and probe).
pub fn with_auth(request: RequestBuilder, credentials: &DoubaoCredentials) -> RequestBuilder {
    request.header(API_KEY_HEADER, &credentials.api_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_api_key_only() {
        let creds = DoubaoCredentials {
            api_key: "doubao-key".into(),
        };
        let request = with_auth(
            reqwest::blocking::Client::new().post("https://example.test/asr"),
            &creds,
        )
        .build()
        .expect("build");
        let headers = request.headers();
        assert_eq!(headers.get(API_KEY_HEADER).unwrap(), "doubao-key");
        assert!(headers.get("X-Api-App-Key").is_none());
        assert!(headers.get("X-Api-Access-Key").is_none());
    }
}
