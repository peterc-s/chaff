#!/usr/bin/env bash
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
REPO_DIR=$(dirname "$SCRIPT_DIR")

cd "$REPO_DIR" || exit

FILES=$(find . -type f \( -name "*.rs" -o -name "*.nix" \))

if [[ -z "$FILES" ]]; then
    echo "No matching files found."
    exit 0
fi

awk '
    FNR == 1 {
        active = 0
        display_name = FILENAME
        sub(/^\//, "", display_name)
        if (FILENAME ~ /\.rs$/) {
            comment_regex = "^[[:space:]]*//"
        } else if (FILENAME ~ /\.nix$/) {
            comment_regex = "^[[:space:]]*#"
        }
    }
    
    /(TODO|FIXME|HACK):/ {
        if (!active) {
            print "\n--- " display_name " ---"
            active = 1
        }
    }

    active {
        if ($0 ~ comment_regex) {
            line = $0
            sub(/^[[:space:]]*/, "", line)
            print line
        } else {
            active = 0
        }
    }
' $FILES
