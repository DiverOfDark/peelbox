#!/usr/bin/env bash
# Sparse-clones examples/ directories from external repos into target/external/.
# Reads repo URLs and pinned commits from compat-sources.toml.
# Idempotent — skips repos already at the correct commit.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG="$ROOT_DIR/compat-sources.toml"
TARGET_DIR="$ROOT_DIR/target/external"

if [ ! -f "$CONFIG" ]; then
  echo "Error: $CONFIG not found" >&2
  exit 1
fi

# Simple TOML value extractor (avoids extra dependencies).
# Usage: get_config <section> <key>
get_config() {
  awk -v section="$1" -v key="$2" '
    /^\[/ { in_section = ($0 == "[" section "]") }
    in_section && $0 ~ "^" key " *=" {
      gsub(/.*= *"/, ""); gsub(/".*/, ""); print; exit
    }
  ' "$CONFIG"
}

fetch_examples() {
  local name=$1
  local repo
  repo=$(get_config "$name" repo)
  local commit
  commit=$(get_config "$name" commit)

  if [ -z "$repo" ] || [ -z "$commit" ]; then
    echo "Error: missing repo or commit for [$name] in $CONFIG" >&2
    return 1
  fi

  local dest="$TARGET_DIR/$name"

  if [ -d "$dest/.git" ]; then
    local current
    current=$(cd "$dest" && git rev-parse HEAD 2>/dev/null || echo "")
    if [ "$current" = "$commit" ]; then
      echo "$name: up to date ($commit)"
      return 0
    fi
    echo "$name: commit changed ($current -> $commit), re-fetching..."
    rm -rf "$dest"
  fi

  echo "$name: fetching $commit from $repo ..."
  mkdir -p "$dest"
  (
    cd "$dest"
    git init -q
    git remote add origin "$repo"
    git sparse-checkout set examples/
    git fetch --depth 1 origin "$commit" -q
    git checkout FETCH_HEAD -q
  )
  echo "$name: done"
}

mkdir -p "$TARGET_DIR"

fetch_examples railpack
fetch_examples nixpacks

echo "All compat fixtures fetched."
