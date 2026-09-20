# log filter pipe

`lfp` - Use an LLM as a filter in a pipeline and keep only the output you care about (such as permission-related messages or error line numbers). The main goal is to prevent LLM tool-calling results from being mixed with unnecessary information and wasting tokens.

- **Raw output backup**: stdin is backed up in full to `raw-dir`, and the backup location is printed to stderr when filtering succeeds.
- **Fail-open**: If the call fails (connection failure, timeout, or invalid response), the original content is printed and the exit code remains 0.
- **Exit code passthrough**: `lfp` does not override the upstream command's exit code. Combined with Bash's `set -o pipefail`, this preserves the correct result.

> This repo contains LLM-assisted code — please thoroughly review and validate it for quality, correctness, and security prior to use.

## Usage

```bash
lfp -p <PROMPT> [OPTIONS]
```

Run `lfp --help` for the full option list.

| Option            | Description                                                                              | Environment variable | Default                  |
| ----------------- | ---------------------------------------------------------------------------------------- | -------------------- | ------------------------ |
| `-p, --prompt`    | Filtering instructions (required)                                                        | -                    | -                        |
| `-m, --model`     | LLM model name (required)                                                                | `LFP_MODEL`          | -                        |
| `--base-url`      | LLM base URL                                                                             | `LFP_BASE_URL`       | `http://localhost:11434` |
| `--num-ctx`       | Ollama `options.num_ctx`                                                                 | `LFP_NUM_CTX`        | Not set                  |
| `-t, --timeout`   | Timeout in seconds for each LLM request                                                  | `LFP_TIMEOUT`        | `30`                     |
| `--system-prompt` | System prompt sent to the LLM                                                            | `LFP_SYSTEM_PROMPT`  | Built-in default         |
| `--min-bytes`     | Minimum input size (bytes) to trigger filtering; `0` always filters                      | `LFP_MIN_BYTES`      | `200`                    |
| `--max-bytes`     | Maximum input size (bytes) to trigger filtering                                          | `LFP_MAX_BYTES`      | Not set                  |
| `--raw-dir`       | Directory for storing raw input                                                          | -                    | `$TMPDIR/lfp`            |
| `--passthrough`   | Always output the original input while still calling the LLM and writing to `runs.jsonl` | -                    | Not set                  |
| `--source`        | Source command for the raw input, written to `runs.jsonl` for later analysis             | -                    | Not set                  |
| `-q, --quiet`     | Suppress the `[lfp] output filtered` stderr diagnostic printed on success                | -                    | Not set                  |

> CLI flags take precedence over environment variables.

### Example

```bash
docker compose logs | lfp --prompt 'keep permission-related information' --model gemma4:26b
```

### Exit Code Passthrough

`lfp` always exits with code 0 during normal execution, including fail-open handling, and does not override the upstream command's exit code. To make the pipeline's exit code reflect the upstream command's result, use `set -o pipefail`:

```bash
set -o pipefail
docker compose logs | lfp --prompt 'keep permission-related information' --model gemma4:26b
echo $?   # Reflects the result of docker compose logs, not lfp
```

### Hook

`lfp` can be wired into a [Claude Code `PreToolUse` hook](https://code.claude.com/docs/en/hooks#pretooluse) to filter noisy `Bash` output (such as build or install logs) before it reaches the model's context. The hook rewrites the matched command to pipe its output through `lfp`. Use `--quiet` here so the agent doesn't see the raw log path in the diagnostic message and go read it directly, defeating the filtering.

`~/.claude/hooks/lfp-shadow.sh`:

```bash
#!/bin/bash
set -euo pipefail

prompt="${1:?missing prompt argument}"
input=$(cat)
command=$(jq -r '.tool_input.command' <<<"$input")

new_command=$(jq -rn \
  --arg cmd "$command" \
  --arg prompt "$prompt" \
  '"set -o pipefail; " + $cmd + " 2>&1 | lfp --passthrough --quiet --source " + ($cmd | @sh) + " -p " + ($prompt | @sh)')

jq -n --arg cmd "$new_command" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "allow",
    updatedInput: { command: $cmd }
  }
}'
```

`~/.claude/settings.json`:

```json
{
  "env": {
    "LFP_MODEL": "qwen3-coder:30b"
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "if": "Bash(cargo build*)",
            "command": "/Users/you/.claude/hooks/lfp-shadow.sh",
            "args": ["Keep only compiler errors and warnings, plus the final build result."]
          }
        ]
      }
    ]
  }
}
```

## Development

```bash
cargo build

cargo fmt --check
cargo check
cargo test
```

### Run Records

In addition to the raw log (the backup of stdin), each `lfp` run appends one JSON Lines record to `{raw-dir}/runs.jsonl`, so filtering results can be evaluated and prompts/models compared later. See [`RunRecord`](src/runlog.rs) for the exact fields. Write failures for this log are silently ignored and do not affect the main flow.

### Shadow Deployment

Collect data before going live: run with `--passthrough` for a while, check `runs.jsonl` for quality, then drop the flag to apply filtering.

```bash
docker compose logs | lfp --prompt 'keep permission-related information' --model gemma4:26b --passthrough
```

## Benchmarks

[readme](./benchmarks/README.md)

## TODO

- [ ] Does not support never-ending follow modes such as `docker compose logs -f`, and does not chunk very large inputs; it always reads the entire input through EOF before processing.
- [ ] Ollama is currently the only supported LLM backend.
- [ ] Files under `raw-dir` are not cleaned up or rotated. By default, they are stored in the system temporary directory and left for the OS to reclaim; use `--raw-dir` to point to another location if you want to keep them long term.
