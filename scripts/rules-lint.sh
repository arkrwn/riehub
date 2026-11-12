#!/usr/bin/env bash
# rules-lint.sh: cheap, mechanical checks for the rules in AGENTS.md and
# the invariants in docs/SPEC.md that a compiler does not catch.
# Part of `make check`. Uses only git and grep so it runs the same on
# Linux, macOS and CI.
#
# To allow one line on purpose, end it with a comment that contains
# `rules-lint: allow <rule>` and a reason, for example:
#   println!("{version}"); // rules-lint: allow print - CLI output
set -uo pipefail

cd "$(git rev-parse --show-toplevel)"
fail=0

# hits <rule> <description> <extended-regex> <pathspec>...
hits() {
  local rule="$1" what="$2" re="$3"; shift 3
  local out
  local flags="-nIE"
  [ "$rule" = "aimark" ] && flags="-niIE"
  out="$(git grep --untracked $flags "$re" -- "$@" 2>/dev/null | grep -v "rules-lint: allow $rule" || true)"
  if [ -n "$out" ]; then
    echo "rules-lint [$rule] $what"
    printf '%s\n' "$out" | head -n 20
    fail=1
  fi
}

RUST=('crates/*.rs' 'src/*.rs')

# S2: nothing prints or logs outside the logging allowlist.
hits print "use tracing with allowlisted fields, not print macros" \
  '(^|[^a-z_])(println!|eprintln!|print!|eprint!|dbg!)\(' "${RUST[@]}"

# S2: log fields that would carry content or credentials.
hits logfield "log call names a content or credential field" \
  '(trace|debug|info|warn|error)!\((.*[^a-z_])?(body|prompt|content|password|passphrase|secret|token|authorization|cookie|api_key)[ ]*=' "${RUST[@]}"

# S10: no unsafe code in riehub crates.
hits unsafe "unsafe code is forbidden (ADR 0002)" \
  '(^|[^a-z_])unsafe[ ]+(fn|impl|trait|extern|\{)' "${RUST[@]}"

# Platform rule: Windows is never supported.
hits windows "no Windows code paths (ADR 0002)" \
  'cfg\((windows|target_os *= *"windows"|target_family *= *"windows")' "${RUST[@]}"

# S8/S11: the web UI loads nothing from a third-party origin.
hits thirdparty "the web UI must not reference third-party origins" \
  '(https?:)?//(fonts\.googleapis|fonts\.gstatic|cdn\.|unpkg\.com|cdnjs\.|ajax\.googleapis|www\.googletagmanager|plausible\.io)' \
  'web/index.html' 'web/src/*'

# R3: no real-looking secrets anywhere in the tree.
hits secret "looks like a real credential, use an obvious placeholder" \
  '(sk-[A-Za-z0-9_-]{24,}|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{30,}|xox[baprs]-[A-Za-z0-9-]{10,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|AIza[0-9A-Za-z_-]{35})' \
  ':!Cargo.lock' ':!web/package-lock.json' ':!scripts/rules-lint.sh'

# R3: secret-bearing files are never tracked.
tracked="$(git ls-files --cached --others --exclude-standard | grep -E '(^|/)\.env(\..*)?$|\.(pem|p12|key|cred)$' | grep -v '\.env\.example$' || true)"
if [ -n "$tracked" ]; then
  echo "rules-lint [secretfile] secret-bearing files must not be in the tree"
  printf '%s\n' "$tracked"
  fail=1
fi

# R1, strict: no AI signature or mark in any file. A product name used
# as a subject is fine, a signature is not. The two scripts that hold
# these patterns are the only exclusions.
hits aimark "no AI signature or mark of any kind (AGENTS.md rule R1)" \
  '(co-authored-by:[ ]*[^ `]|(generated|written|authored|created|assisted)[ -](with|by):?[ ]+(\[|an?[ ]+)?(ai|llm|claude|chatgpt|gpt|copilot|gemini|codex|cursor|deepseek|aider|anthropic|openai)|generated with \[|assisted-by:|🤖|noreply@anthropic|noreply@openai)' \
  '.' ':!scripts/rules-lint.sh' ':!factory/factory.sh'

# Writing style (AGENTS.md section 8), strict: no em-dash in any file,
# whatever its type. The character is built from its bytes so that this
# script does not contain one either.
EMDASH="$(printf '\342\200\224')"
hits emdash "no em-dash anywhere, use ' - ' or a new sentence" \
  "$EMDASH" '.' ':!web/src/assets/fonts/*'

# Writing style, strict: no semicolon in prose. Checked in Markdown,
# outside code fences, inline code, URLs and HTML entities. Legal texts
# keep their own punctuation. To allow one line on purpose, add the
# HTML comment `<!-- rules-lint: allow semicolon -->` to it.
semi="$(git ls-files --cached --others --exclude-standard -- '*.md' ':!CLA.md' ':!LICENSE*' | while IFS= read -r f; do
  [ -f "$f" ] || continue
  awk -v f="$f" '
    /^[ \t]*```/ { code = !code; next }
    code { next }
    /rules-lint: allow semicolon/ { next }
    {
      t = $0
      gsub(/`[^`]*`/, "", t)
      gsub(/https?:\/\/[^ )>]*/, "", t)
      gsub(/&[a-z]+;|&#[0-9]+;/, "", t)
      if (index(t, ";")) printf "%s:%d:%s\n", f, NR, $0
    }' "$f"
done)"
if [ -n "$semi" ]; then
  echo "rules-lint [semicolon] no semicolons in prose, use a comma or a new sentence"
  printf '%s\n' "$semi" | head -n 20
  fail=1
fi

# Every riehub crate forbids unsafe at the crate root.
for root in crates/*/src/lib.rs crates/*/src/main.rs; do
  [ -f "$root" ] || continue
  case "$root" in */main.rs) [ -f "${root%main.rs}lib.rs" ] && continue ;; esac
  if ! grep -q '^#!\[forbid(unsafe_code)\]' "$root"; then
    echo "rules-lint [forbid] $root lacks #![forbid(unsafe_code)]"
    fail=1
  fi
done

if [ "$fail" -eq 0 ]; then echo "rules-lint: ok"; fi
exit "$fail"
