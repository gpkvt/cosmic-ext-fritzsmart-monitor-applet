#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  echo 'Usage: make chatgpt-change REQUEST="Describe the requested change"' >&2
  exit 2
}
[[ $# -ge 1 ]] || usage
request="$*"
root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "Run this from inside the repository checkout." >&2
  exit 1
}
cd "$root"

branch="$(git branch --show-current)"
[[ -n "$branch" ]] || branch="(detached HEAD)"
status="$(git status --short)"
recent="$(git log -5 --oneline)"
diff="$(git diff --no-ext-diff -- .)"
staged="$(git diff --cached --no-ext-diff -- .)"
untracked="$(git ls-files --others --exclude-standard)"

# Keep clipboard payload practical; users can add more context in ChatGPT.
limit=18000
if (( ${#diff} > limit )); then
  diff="${diff:0:limit}

[Diff truncated at ${limit} characters. Inspect the full local diff before sharing.]"
fi
if (( ${#staged} > limit )); then
  staged="${staged:0:limit}

[Staged diff truncated at ${limit} characters.]"
fi

prompt_file="${TMPDIR:-/tmp}/fritz-power-monitor-chatgpt-request-$$.md"
{
  cat <<EOF
# FRITZ!Power Monitor change request

Please implement this change in the existing repository. Do not merely provide advice or a proposed patch.

## Requested change
$request

## Repository context
- Repository: https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet
- Branch: $branch
- Project: Rust COSMIC desktop applet
- Follow the existing architecture, COSMIC design conventions, translations and tests.
- Preserve unrelated local changes. Do not overwrite or discard user work.
- Add or update tests and documentation where appropriate.
- Run formatting, check, clippy and tests; fix failures before proposing a PR.
- Explain any checks that could not be run. Do not claim unverified success.
- Keep changes focused and avoid adding paid services or API dependencies.

## Recent commits
```
$recent
```

## Working tree status
```
${status:-clean}
```

## Untracked files (names only)
```
${untracked:-none}
```

## Staged diff
```diff
${staged:-No staged changes}
```

## Unstaged diff
```diff
${diff:-No unstaged tracked changes}
```

Review the supplied context, identify the relevant files, then implement the requested change. If essential information is missing, ask one concise question before making assumptions.
EOF
} > "$prompt_file"

copied=0
if command -v wl-copy >/dev/null 2>&1; then
  wl-copy < "$prompt_file" && copied=1
elif command -v xclip >/dev/null 2>&1; then
  xclip -selection clipboard < "$prompt_file" && copied=1
elif command -v xsel >/dev/null 2>&1; then
  xsel --clipboard --input < "$prompt_file" && copied=1
fi

echo "Prompt prepared: $prompt_file"
if (( copied )); then
  echo "Copied the prompt to your clipboard."
else
  echo "No clipboard utility found; open the prompt file and copy its contents."
fi

if command -v xdg-open >/dev/null 2>&1; then
  xdg-open https://chatgpt.com/ >/dev/null 2>&1 &
  echo "Opened ChatGPT in your browser."
else
  echo "Open https://chatgpt.com/ and paste the prompt manually."
fi
