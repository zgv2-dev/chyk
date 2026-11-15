#!/usr/bin/env bash
# Print the CHANGELOG.md section for one version (without its heading), for release notes.
#   scripts/release-notes.sh 1.3.2      or      scripts/release-notes.sh v1.3.2
set -euo pipefail
version="${1#v}"
awk -v v="$version" '
  /^## / { if (found) exit; found = ($2 == v) ; next }
  found { print }
' "$(dirname "$0")/../CHANGELOG.md" | sed -e '/./,$!d'
