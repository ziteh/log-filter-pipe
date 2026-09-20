# lfp benchmarks

Use output from real CLI tools as test data to benchmark filtering accuracy and runtime across different models and prompts, making it easier to choose a model and compare results.

## Usage

```bash
cargo build
benchmarks/run.sh <model> [base-url] [timeout-secs]

# Example
benchmarks/run.sh qwen3-coder:30b http://192.168.50.80:11434 120
```

The benchmark prints PASS/FAIL, difficulty, and runtime for each case. For failures, it lists what was missing or leaked. It then reports pass/fail counts and total runtime by difficulty (easy/medium/hard), as well as overall totals.

## Case Design

Each `cases/*.case` file defines one test case:

```text
FIXTURE: <filename under fixtures/, the raw input for this case>
DIFFICULTY: easy|medium|hard
PROMPT: <filtering instructions>
KEEP:
<content that should appear in the output, one item per line; may be a full line or representative substring>
DROP:
<content that should not appear in the output, one item per line>
```

The benchmark passes the entire raw file referenced by `FIXTURE` to `lfp --prompt <PROMPT> --model <model>`, then checks the output: every item listed under `KEEP` must appear, and no item listed under `DROP` may appear. Lines not listed under
`KEEP` or `DROP` are not checked—whether they should be kept can be subjective, and enforcing a decision could incorrectly reject a valid answer.

## Difficulty Levels

- **easy**: Keyword or substring matching (for example, “keep only messages at the `[ERROR]` level”).
- **medium**: Requires semantic classification, cross-line summarization, or somewhat more complex conditions (for example, “keep only permission- or authentication-related errors, not general info messages”).
- **hard**: Requires precise multiline block-boundary detection, numerical or logical reasoning, or strict exclusion of distractors (for example, “keep only the complete exception traceback block and exclude all other errors,” “keep messages with line numbers > 20,” or reasoning about permissions at the bit level, such as “the owner lacks `w` but the group has `w`”).

To add a new case, create a `.case` file using the format above. To add a new scenario, first add a raw input file under `fixtures/`, then write several `.case` files for it at different difficulty levels.
