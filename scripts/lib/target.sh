# shellcheck shell=bash
# Sourced by the build scripts: $bin is where cargo writes binaries, which a worktree may share with the main
# checkout (scripts/share-target.sh).
bin=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
