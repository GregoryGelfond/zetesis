#!/bin/sh
# A missing prior file permits adoption; an unreadable existing file never does.
set -eu
repo_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd -- "$repo_dir"
previous=$(scripts/maintenance.sh coverage-previous --event "$GITHUB_EVENT_PATH")
if [ -z "$previous" ]; then
    exit 0
fi
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
floor_path=scripts/coverage-floor.txt
if git show "$previous:$floor_path" > "$scratch/previous"; then
    scripts/maintenance.sh coverage-ratchet --previous "$scratch/previous" --current "$floor_path"
else
    git ls-tree "$previous" -- "$floor_path" > "$scratch/entries"
    if [ -s "$scratch/entries" ]; then
        printf '%s\n' 'Could not read the previous coverage floor.' >&2
        exit 1
    fi
fi
