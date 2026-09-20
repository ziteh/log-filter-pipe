mod cli;
mod llm;
mod runlog;
mod tee;

use std::io::Read;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cli::Config;
use tee::TeeReader;

fn main() -> std::process::ExitCode {
    let cfg = match Config::parse() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("[lfp] {err}");
            return std::process::ExitCode::from(2);
        }
    };

    run(&cfg);
    std::process::ExitCode::SUCCESS
}

fn run(cfg: &Config) {
    let base_log_name = base_log_name();
    let raw_log_path = cfg.raw_dir.join(format!("{base_log_name}.raw.log"));
    let input = read_stdin(&raw_log_path);

    if input.is_empty() {
        return;
    }

    if input.len() < cfg.min_bytes {
        print!("{input}");

        let record = runlog::RunRecord::skipped(
            cfg,
            raw_log_path.display().to_string(),
            input.len(),
            "below_min_bytes",
        );
        runlog::append(&cfg.raw_dir, &record);
        return;
    }

    if cfg.max_bytes.is_some_and(|max| input.len() > max) {
        print!("{input}");

        let record = runlog::RunRecord::skipped(
            cfg,
            raw_log_path.display().to_string(),
            input.len(),
            "above_max_bytes",
        );
        runlog::append(&cfg.raw_dir, &record);
        return;
    }

    let started = Instant::now();
    let timeout = Duration::from_secs(cfg.timeout_secs);
    let result = llm::generate(
        &cfg.base_url,
        &cfg.model,
        &cfg.system_prompt,
        &cfg.prompt,
        &input,
        cfg.num_ctx,
        timeout,
    );
    let elapsed_ms = started.elapsed().as_millis();

    match result {
        Ok(filtered) => {
            if cfg.passthrough {
                print!("{input}");
            } else {
                print!("{filtered}");
            }
            eprintln!(
                "[lfp] output filtered; raw input: {}",
                raw_log_path.display()
            );

            let filtered_log_path = cfg.raw_dir.join(format!("{base_log_name}.filtered.log"));
            let filtered_log_path = std::fs::write(&filtered_log_path, &filtered)
                .ok()
                .map(|()| filtered_log_path.display().to_string());

            let record = runlog::RunRecord::ok(
                cfg,
                raw_log_path.display().to_string(),
                filtered_log_path,
                input.len(),
                filtered.len(),
                elapsed_ms,
            );
            runlog::append(&cfg.raw_dir, &record);
        }
        Err(err) => {
            print!("{input}");

            let record = runlog::RunRecord::fail_open(
                cfg,
                raw_log_path.display().to_string(),
                input.len(),
                elapsed_ms,
                err.reason.as_str(),
                err.detail,
            );
            runlog::append(&cfg.raw_dir, &record);
        }
    }
}

/// Base file name (without extension) shared by the raw input and filtered output
/// backups for this execution, e.g. `lfp-{epoch_millis}-{pid}`.
fn base_log_name() -> String {
    let epoch_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    format!("lfp-{epoch_millis}-{pid}")
}

/// Read the entire stdin and attempt to tee the raw bytes to `raw_log_path`.
/// If opening or writing the file fails,
/// silently perform a `fail-open` and continue running using the standard stdin.
fn read_stdin(raw_log_path: &std::path::Path) -> String {
    let stdin = std::io::stdin();
    let mut buf = Vec::new();

    match create_raw_log_file(raw_log_path) {
        Some(file) => {
            let _ = TeeReader::new(stdin.lock(), file).read_to_end(&mut buf);
        }
        None => {
            let _ = stdin.lock().read_to_end(&mut buf);
        }
    }

    String::from_utf8_lossy(&buf).into_owned()
}

/// Create (or verify that it is writable) `raw-dir` and open the current raw log file;
/// if any step fails, return `None`, leaving it to the caller to silently perform a `fail-open`.
fn create_raw_log_file(raw_log_path: &std::path::Path) -> Option<std::fs::File> {
    let parent = raw_log_path.parent()?;
    std::fs::create_dir_all(parent).ok()?;
    std::fs::File::create(raw_log_path).ok()
}
