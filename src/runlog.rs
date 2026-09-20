use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::cli::Config;

#[derive(Serialize)]
struct RunArgs<'a> {
    prompt: &'a str,
    model: &'a str,
    base_url: &'a str,
    num_ctx: Option<u32>,
    timeout: u64,
    min_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<&'a str>,
}

/// An execution record for a single `lfp` call, corresponding to a line in `runs.jsonl`.
#[derive(Serialize)]
pub struct RunRecord<'a> {
    ts: u64,
    pid: u32,
    version: &'static str,
    args: RunArgs<'a>,
    system_prompt: &'a str,
    raw_log_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filtered_log_path: Option<String>,
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
        filtered_log_path: Option<String>,
        input_bytes: usize,
        output_bytes: usize,
        elapsed_ms: u128,
    ) -> Self {
        Self::new(
            cfg,
            raw_log_path,
            filtered_log_path,
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
            None,
            input_bytes,
            input_bytes,
            elapsed_ms,
            "fail_open",
            Some(reason),
            Some(error_detail),
        )
    }

    pub fn skipped(
        cfg: &'a Config,
        raw_log_path: String,
        input_bytes: usize,
        reason: &'static str,
    ) -> Self {
        Self::new(
            cfg,
            raw_log_path,
            None,
            input_bytes,
            input_bytes,
            0,
            "skipped",
            Some(reason),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        cfg: &'a Config,
        raw_log_path: String,
        filtered_log_path: Option<String>,
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
            version: env!("CARGO_PKG_VERSION"),
            args: RunArgs {
                prompt: &cfg.prompt,
                model: &cfg.model,
                base_url: &cfg.base_url,
                num_ctx: cfg.num_ctx,
                timeout: cfg.timeout_secs,
                min_bytes: cfg.min_bytes,
                max_bytes: cfg.max_bytes,
                source: cfg.source.as_deref(),
            },
            system_prompt: &cfg.system_prompt,
            raw_log_path,
            filtered_log_path,
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
            system_prompt: crate::cli::DEFAULT_SYSTEM_PROMPT.to_string(),
            min_bytes: 0,
            max_bytes: None,
            raw_dir: PathBuf::from("/tmp/lfp"),
            passthrough: false,
            source: None,
        }
    }

    #[test]
    fn ok_record_serializes_without_reason_and_error_detail() {
        let cfg = test_config();
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), None, 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["outcome"], "ok");
        assert!(json.get("reason").is_none());
        assert!(json.get("error_detail").is_none());
    }

    #[test]
    fn args_omits_max_bytes_when_absent() {
        let cfg = test_config();
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), None, 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert!(json["args"].get("max_bytes").is_none());
    }

    #[test]
    fn args_includes_max_bytes_when_present() {
        let mut cfg = test_config();
        cfg.max_bytes = Some(10_000);
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), None, 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["args"]["max_bytes"], 10_000);
    }

    #[test]
    fn skipped_record_serializes_given_reason() {
        let cfg = test_config();
        let record = RunRecord::skipped(
            &cfg,
            "/tmp/lfp/lfp-1-2.log".to_string(),
            5,
            "above_max_bytes",
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["outcome"], "skipped");
        assert_eq!(json["reason"], "above_max_bytes");
    }

    #[test]
    fn args_omits_source_when_absent() {
        let cfg = test_config();
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), None, 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert!(json["args"].get("source").is_none());
    }

    #[test]
    fn args_includes_source_when_present() {
        let mut cfg = test_config();
        cfg.source = Some("docker compose logs".to_string());
        let record = RunRecord::ok(&cfg, "/tmp/lfp/lfp-1-2.log".to_string(), None, 10, 5, 100);

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["args"]["source"], "docker compose logs");
    }

    #[test]
    fn ok_record_includes_filtered_log_path_when_present() {
        let cfg = test_config();
        let record = RunRecord::ok(
            &cfg,
            "/tmp/lfp/lfp-1-2.log".to_string(),
            Some("/tmp/lfp/lfp-1-2.filtered.log".to_string()),
            10,
            5,
            100,
        );

        let json = serde_json::to_value(&record).unwrap();

        assert_eq!(json["filtered_log_path"], "/tmp/lfp/lfp-1-2.filtered.log");
    }

    #[test]
    fn fail_open_record_omits_filtered_log_path() {
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

        assert!(json.get("filtered_log_path").is_none());
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
