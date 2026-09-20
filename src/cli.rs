use std::env;

use clap::Parser;

const DEFAULT_BASE_URL: &str = "http://localhost:11434";

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Parser, Debug)]
#[command(
    name = "lfp",
    about = "A pipeline tool that uses an LLM to filter CLI command output"
)]
pub struct RawArgs {
    /// Filter prompts.
    #[arg(short, long)]
    pub prompt: String,

    /// Model Name, fallback to the env LFP_MODEL.
    #[arg(short, long)]
    pub model: Option<String>,

    /// LLM base URL (e.g., http://localhost:11434), fallback to the env LFP_BASE_URL.
    #[arg(long)]
    pub base_url: Option<String>,

    /// Context window, fallback to the env LFP_NUM_CTX.
    #[arg(long)]
    pub num_ctx: Option<u32>,

    /// Timeout in seconds for a single request, fallback to the env LFP_TIMEOUT.
    #[arg(short, long)]
    pub timeout: Option<u64>,

    /// Raw log directory, fallback to std::env::temp_dir()/lfp.
    #[arg(long)]
    pub raw_dir: Option<std::path::PathBuf>,

    /// Always output the original input as-is,
    /// while still calling the LLM and logging the execution details (shadow deployment).
    #[arg(long)]
    pub passthrough: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub prompt: String,
    pub model: String,
    pub base_url: String,
    pub num_ctx: Option<u32>,
    pub timeout_secs: u64,
    pub raw_dir: std::path::PathBuf,
    pub passthrough: bool,
}

impl Config {
    /// Parse CLI parameters and apply environment variable fallbacks (flags take precedence).
    pub fn parse() -> anyhow::Result<Self> {
        let args = RawArgs::parse();

        let model = args
            .model
            .or_else(|| env::var("LFP_MODEL").ok())
            .ok_or_else(|| {
                anyhow::anyhow!("Must specify --model or set environment variable LFP_MODEL")
            })?;

        let base_url = args
            .base_url
            .or_else(|| env::var("LFP_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let num_ctx = match args.num_ctx {
            Some(n) => Some(n),
            None => env::var("LFP_NUM_CTX")
                .ok()
                .map(|s| s.parse())
                .transpose()
                .map_err(|e| {
                    anyhow::anyhow!("Environment variable LFP_NUM_CTX is not a valid number: {e}")
                })?,
        };

        let timeout_secs = match args.timeout {
            Some(t) => t,
            None => match env::var("LFP_TIMEOUT").ok() {
                Some(s) => s.parse().map_err(|e| {
                    anyhow::anyhow!("Environment variable LFP_TIMEOUT is not a valid number: {e}")
                })?,
                None => DEFAULT_TIMEOUT_SECS,
            },
        };

        let raw_dir = args.raw_dir.unwrap_or_else(|| env::temp_dir().join("lfp"));

        Ok(Self {
            prompt: args.prompt,
            model,
            base_url,
            num_ctx,
            timeout_secs,
            raw_dir,
            passthrough: args.passthrough,
        })
    }
}
