#!/usr/bin/env bash
# Renames a copy of this template to a new game, then deletes itself. Usage: scripts/rename.sh my-game
set -euo pipefail
cd "$(dirname "$0")/.."
name="${1:-}"
# The device's pidof and killall match the 15-character process name.
if ! [[ "$name" =~ ^[a-z][a-z0-9]*(-[a-z0-9]+)*$ ]] || [ ${#name} -gt 15 ]; then
  echo "usage: scripts/rename.sh <name>  (lowercase letters, digits and hyphens, at most 15 characters)" >&2
  exit 1
fi
if ! git grep -qi twinbit -- . ':!scripts/rename.sh'; then
  echo "nothing to rename: no twinbit references left" >&2
  exit 1
fi

snake="${name//-/_}"
upper="$(tr 'a-z' 'A-Z' <<<"$snake")"
pascal="$(perl -pe 's/(^|-)(\w)/\U$2/g' <<<"$name")"
title="$(tr 'a-z-' 'A-Z ' <<<"$name")"

perl -0pi -e "s/^# twinbit-ds\n/# $name\n/; s/<!-- template -->.*?<!-- \/template -->\n\n//s" README.md
git grep -lIi twinbit -- . ':!scripts/rename.sh' | while read -r f; do
  NAME="$name" SNAKE="$snake" UPPER="$upper" PASCAL="$pascal" TITLE="$title" perl -pi -e '
    s/twinbit_/$ENV{SNAKE}_/g;
    s/"TWINBIT"/"$ENV{TITLE}"/g;
    s/TWINBIT/$ENV{UPPER}/g;
    s/Twinbit/$ENV{PASCAL}/g;
    s/twinbit/$ENV{NAME}/g;
  ' "$f"
done
git mv assets/Twinbit.sh "assets/$pascal.sh"
git mv assets/Twinbit.png "assets/$pascal.png"
cargo update --workspace --quiet
git rm -q scripts/rename.sh
git add -A
echo "renamed to $name; review with git diff --cached, then commit"
