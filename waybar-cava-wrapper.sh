#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

printf '%s\n' "{\"text\":\"<span color='#888888'>▁▁▁▁</span>\",\"class\":\"silent\",\"percentage\":0}"

cava -p ~/.config/cava/config \
  | "$SCRIPT_DIR/target/release/waybar-cavars" \
  | while IFS= read -r line; do
      printf '%s\n' "$line" | tee /dev/null
    done
