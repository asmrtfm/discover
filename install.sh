#!/usr/bin/env bash

[[ "${1,,}" != *(\-)@(h)?(elp) ]] || { cat <<EOL
USAGE
        ./install [ -b|--build ]

SYNOPSIS:
        Optionally build, and install to BOTH user and system-wide.
        Automatically builds via bin/build-static if no binary is found.

EOL

  exit 0
}

REPO_ROOT="${BASH_SOURCE[0]%\/*}"
AST_GREP_DIR="$REPO_ROOT/ast-grep"

# Explicit build flag
[[ "${1,,}" != *(\-)@(b)?(uild) ]] || "$REPO_ROOT/bin/build-static"

# Locate the binary from a static build target directory
find_binary() {
  local name="$1"
  local target_dir="$AST_GREP_DIR/target"
  # Static builds go under target/<triple>/release
  for triple in x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc; do
    [[ -x "$target_dir/$triple/release/$name" ]] && echo "$target_dir/$triple/release/$name" && return
  done
  # Fall back to plain cargo build --release
  [[ -x "$target_dir/release/$name" ]] && echo "$target_dir/release/$name" && return
  return 1
}

# Build if no binary exists
if ! find_binary ast-grep &>/dev/null; then
  read -rp "No binary found. Build now? [Y/n] " answer
  if [[ "${answer,,}" != "n" ]]; then
    "$REPO_ROOT/bin/build-static"
  else
    echo "Cannot install without a binary." >&2
    exit 1
  fi
fi

ast_grep="$(find_binary ast-grep)"
sg="$(find_binary sg)"

cp "$ast_grep" ~/.cargo/bin/ast-grep
cp "$sg" ~/.cargo/bin/sg

sudo cp "$ast_grep" /usr/local/bin/ast-grep
sudo cp "$sg" /usr/local/bin/sg
