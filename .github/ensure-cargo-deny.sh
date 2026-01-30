#!/usr/bin/env bash
set -euo pipefail

required_version="0.19.0"

version_ge() {
  local IFS=.
  local -a current=($1)
  local -a required=($2)
  local i

  for ((i=0; i<${#current[@]} || i<${#required[@]}; i++)); do
    local c=${current[i]:-0}
    local r=${required[i]:-0}
    if ((c > r)); then
      return 0
    fi
    if ((c < r)); then
      return 1
    fi
  done

  return 0
}

if ! command -v cargo-deny >/dev/null 2>&1; then
  cargo install cargo-deny --version "$required_version" --locked
  exit 0
fi

current_version="$(cargo deny --version | awk '{print $2}')"
if ! version_ge "$current_version" "$required_version"; then
  cargo install cargo-deny --version "$required_version" --locked
fi
