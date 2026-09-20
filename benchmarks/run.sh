
set -eo pipefail

MODEL="${1:?usage: run.sh <model> [base-url] [timeout-secs]}"
BASE_URL="${2:-http://localhost:11434}"
TIMEOUT="${3:-120}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${LFP_BIN:-$SCRIPT_DIR/../target/debug/lfp}"
RAW_DIR="$(mktemp -d)"

if [[ ! -x "$BIN" ]]; then
    echo "lfp executable not found: $BIN (run cargo build first, or set its path with LFP_BIN)" >&2
    exit 1
fi

pass=0
fail=0
total_elapsed=0
easy_pass=0; easy_fail=0
medium_pass=0; medium_fail=0
hard_pass=0; hard_fail=0

for case_file in "$SCRIPT_DIR"/cases/*.case; do
    case_name=$(basename "$case_file" .case)
    fixture=$(sed -n 's/^FIXTURE: //p' "$case_file")
    difficulty=$(sed -n 's/^DIFFICULTY: //p' "$case_file")
    prompt=$(sed -n 's/^PROMPT: //p' "$case_file")
    fixture_path="$SCRIPT_DIR/fixtures/$fixture"

    keep=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && keep+=("$line")
    done < <(sed -n '/^KEEP:$/,/^DROP:$/p' "$case_file" | sed '1d;$d')

    drop=()
    while IFS= read -r line; do
        [[ -n "$line" ]] && drop+=("$line")
    done < <(sed -n '/^DROP:$/,$p' "$case_file" | tail -n +2)

    start=$SECONDS
    output=$(cat "$fixture_path" | "$BIN" --prompt "$prompt" --model "$MODEL" \
        --base-url "$BASE_URL" --raw-dir "$RAW_DIR" --timeout "$TIMEOUT" 2>/dev/null)
    elapsed=$((SECONDS - start))
    total_elapsed=$((total_elapsed + elapsed))

    ok=true
    problems=()
    for k in "${keep[@]}"; do
        if ! grep -qF -- "$k" <<<"$output"; then
            ok=false
            problems+=("missing (should keep): $k")
        fi
    done
    for d in "${drop[@]}"; do
        if grep -qF -- "$d" <<<"$output"; then
            ok=false
            problems+=("leaked (should drop): $d")
        fi
    done

    if $ok; then
        echo "[PASS] $case_name [$difficulty] (${elapsed}s)"
        pass=$((pass + 1))
        tier_delta_pass=1
        tier_delta_fail=0
    else
        echo "[FAIL] $case_name [$difficulty] (${elapsed}s)"
        for p in "${problems[@]}"; do
            echo "    $p"
        done
        fail=$((fail + 1))
        tier_delta_pass=0
        tier_delta_fail=1
    fi

    case "$difficulty" in
    easy)
        easy_pass=$((easy_pass + tier_delta_pass))
        easy_fail=$((easy_fail + tier_delta_fail))
        ;;
    medium)
        medium_pass=$((medium_pass + tier_delta_pass))
        medium_fail=$((medium_fail + tier_delta_fail))
        ;;
    hard)
        hard_pass=$((hard_pass + tier_delta_pass))
        hard_fail=$((hard_fail + tier_delta_fail))
        ;;
    esac
done

echo
echo "By difficulty:"
echo "  easy:   pass=$easy_pass   fail=$easy_fail"
echo "  medium: pass=$medium_pass fail=$medium_fail"
echo "  hard:   pass=$hard_pass   fail=$hard_fail"
echo
echo "model=$MODEL  pass=$pass  fail=$fail  total_elapsed=${total_elapsed}s  raw_dir=$RAW_DIR"
