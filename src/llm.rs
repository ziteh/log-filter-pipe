use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Low temperature for extraction-style filtering.
const TEMPERATURE: f32 = 0.1;

/// /api/generate request body.
#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    /// Disable thinking output for models that support it.
    think: bool,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
}

/// /api/generate response body.
#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Clone, Copy)]
pub enum FailReason {
    ConnectError,
    Timeout,
    BadStatus,
    ParseError,
}

impl FailReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectError => "connect_error",
            Self::Timeout => "timeout",
            Self::BadStatus => "bad_status",
            Self::ParseError => "parse_error",
        }
    }
}

/// LLM backend error.
#[derive(Debug)]
pub struct LlmError {
    pub reason: FailReason,
    pub detail: String,
}

pub fn generate(
    base_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    input: &str,
    num_ctx: Option<u32>,
    timeout: Duration,
) -> Result<String, LlmError> {
    let full_prompt =
        format!("{system_prompt}\n\nUser Prompt: {user_prompt}\n\nOriginal Content:\n{input}");

    let body = OllamaRequest {
        model,
        prompt: &full_prompt,
        stream: false,
        think: false,
        options: OllamaOptions {
            temperature: TEMPERATURE,
            num_ctx,
        },
    };

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let url = format!("{base_url}/api/generate");
    let mut response = agent
        .post(&url)
        .send_json(&body)
        .map_err(classify_send_error)?;

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(LlmError {
            reason: FailReason::BadStatus,
            detail: format!("HTTP {status}"),
        });
    }

    let text = response.body_mut().read_to_string().map_err(|e| LlmError {
        reason: FailReason::ConnectError,
        detail: e.to_string(),
    })?;

    let parsed: OllamaResponse = serde_json::from_str(&text).map_err(|e| LlmError {
        reason: FailReason::ParseError,
        detail: e.to_string(),
    })?;

    Ok(parsed.response)
}

fn classify_send_error(err: ureq::Error) -> LlmError {
    let reason = match err {
        ureq::Error::Timeout(_) => FailReason::Timeout,
        _ => FailReason::ConnectError,
    };
    LlmError {
        reason,
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_request_omits_num_ctx_when_absent() {
        let req = OllamaRequest {
            model: "llama3.1",
            prompt: "hello",
            stream: false,
            think: false,
            options: OllamaOptions {
                temperature: TEMPERATURE,
                num_ctx: None,
            },
        };

        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["options"].get("num_ctx"), None);
    }

    #[test]
    fn generate_request_always_disables_thinking() {
        let req = OllamaRequest {
            model: "llama3.1",
            prompt: "hello",
            stream: false,
            think: false,
            options: OllamaOptions {
                temperature: TEMPERATURE,
                num_ctx: None,
            },
        };

        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["think"], false);
    }

    #[test]
    fn generate_request_serializes_num_ctx_when_present() {
        let req = OllamaRequest {
            model: "llama3.1",
            prompt: "hello",
            stream: false,
            think: false,
            options: OllamaOptions {
                temperature: TEMPERATURE,
                num_ctx: Some(4096),
            },
        };

        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["options"]["num_ctx"], 4096);
    }

    #[test]
    fn generate_request_serializes_temperature() {
        let req = OllamaRequest {
            model: "llama3.1",
            prompt: "hello",
            stream: false,
            think: false,
            options: OllamaOptions {
                temperature: TEMPERATURE,
                num_ctx: None,
            },
        };

        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["options"]["temperature"], TEMPERATURE);
    }

    #[test]
    fn generate_response_deserializes_response_field() {
        let parsed: OllamaResponse = serde_json::from_str(r#"{"response":"kept lines"}"#).unwrap();

        assert_eq!(parsed.response, "kept lines");
    }

    #[test]
    fn fail_reason_as_str_matches_runlog_naming() {
        assert_eq!(FailReason::ConnectError.as_str(), "connect_error");
        assert_eq!(FailReason::Timeout.as_str(), "timeout");
        assert_eq!(FailReason::BadStatus.as_str(), "bad_status");
        assert_eq!(FailReason::ParseError.as_str(), "parse_error");
    }
}
