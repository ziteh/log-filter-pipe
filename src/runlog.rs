use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::cli::Config;
use crate::llm::SYSTEM_PROMPT;

#[derive(Serialize)]
struct RunArgs<'a> {
    prompt: &'a str,
    model: &'a str,
    base_url: &'a str,
    num_ctx: Option<u32>,
    timeout: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a str>,
}

/// An execution record for a single `lfp` call, corresponding to a line in `runs.jsonl`.
#[derive(Serialize)]
pub struct RunRecord<'a> {
    ts: u64,
    pid: u32,
    args: RunArgs<'a>,
    system_prompt: &'a str,
    raw_log_path: String,
    input_bytes: usize,
    output_bytes: usize,
    elapsed_ms: u128,
    passthrough: bool,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_detail: Option<String>,
}

impl<'a> RunRecord<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn ok(
        cfg: &'a Config,
        raw_log_path: String,
        input_bytes: usize,
        output_bytes: usize,
        elapsed_ms: u128,
    ) -> Self {
        Self::new(
            cfg,
            raw_log_path,
            input_bytes,
            output_bytes,
            elapsed_ms,
            "ok",
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fail_open(
        cfg: &'a Config,
        raw_log_path: String,
        input_bytes: usize,
        elapsed_ms: u128,
        reason: &'static str,
        error_detail: String,
    ) -> Self {
        Self::new(
            cfg,
            raw_log_path,
            input_bytes,
            input_bytes,
            elapsed_ms,
            "fail_open",
            Some(reason),
            Some(error_detail),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        cfg: &'a Config,
        raw_log_path: String,
        input_bytes: usize,
        output_bytes: usize,
        elapsed_ms: u128,
        outcome: &'static str,
        reason: Option<&'static str>,
        error_detail: Option<String>,
    ) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            ts,
            pid: std::process::id(),
            args: RunArgs {
                prompt: &cfg.prompt,
                model: &cfg.model,
                base_url: &cfg.base_url,
                num_ctx: cfg.num_ctx,
                timeout: cfg.timeout_secs,
                source: cfg.source.as_deref(),
            },
            system_prompt: SYSTEM_PROMPT,
            raw_log_path,
            input_bytes,
            output_bytes,
            elapsed_ms,
            passthrough: cfg.passthrough,
            outcome,
            reason,
            error_detail,
        }
    }
}

pub fn append(raw_dir: &Path, record: &RunRecord) {
    let Ok(line) = serde_json::to_string(record) else {
        return;
    };

    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(raw_dir.join("runs.jsonl"))
    else {
        return;
    };

    let _ = writeln!(file, "{line}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            prompt: "keep foo".to_string(),
            model: "llama3.1".to_string(),
            base_url: "http://localhost:11434".to_string(),
            num_ctx: Some(4096),
            timeout_secs: 30,
            raw_dir: PathBuf::from("/tmp/lfp"),
            passthrough: false,
            source: None,
        }
    }

    #[test]
    fn ok_record_serializes_without_reason_and_error_detail() {
        let cfg = test_config();
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["outcome"], "ok");
        assert!(json.get("reason").is_none());
        assert!(json.get("error_detail").is_none());
    }

    #[test]
    fn args_omits_source_when_absent() {
        let cfg = test_config();
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert!(json["args"].get("source").is_none());
    }

    #[test]
    fn args_includes_source_when_present() {
        let mut cfg = test_config();
        cfg.source = Some("docker compose logs".to_string());
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["args"]["source"], "docker compose logs");
    }

    #[test]
    fn fail_open_record_serializes_reason_and_error_detail() {
        let cfg = test_config();
        let record = RunRecord::fail_open(
            &cfg,
            "/tmp/lfp/lfp-1-2.log".to_string(),
            10,
            50,
            "timeout",
            "request timed out".to_string(),
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["outcome"], "fail_open");
        assert_eq!(json["reason"], "timeout");
        assert_eq!(json["error_detail"], "request timed out");
    }
}
