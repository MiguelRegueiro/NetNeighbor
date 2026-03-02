#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <tag-or-version>" >&2
  exit 1
fi

INPUT="$1"
VERSION="${INPUT#v}"
CHANGELOG_FILE="CHANGELOG.md"

if [[ ! -f "$CHANGELOG_FILE" ]]; then
  echo "Error: $CHANGELOG_FILE not found." >&2
  exit 1
fi

extract_section() {
  local version="$1"
  awk -v version="$version" '
    BEGIN {
      in_section = 0;
      found = 0;
      target = "## [" version "]";
    }

    $0 ~ /^## \[/ {
      if (in_section == 1) {
        exit 0;
      }
      if (index($0, target) == 1) {
        in_section = 1;
        found = 1;
      }
    }

    in_section == 1 {
      print $0;
    }

    END {
      if (found == 0) {
        exit 2;
      }
    }
  ' "$CHANGELOG_FILE"
}

if ! SECTION_CONTENT="$(extract_section "${VERSION}")"; then
  echo "Error: Could not find section [${VERSION}] in ${CHANGELOG_FILE}." >&2
  echo "Expected a heading like: ## [${VERSION}] - YYYY-MM-DD" >&2
  exit 1
fi

if [[ -z "${SECTION_CONTENT//[[:space:]]/}" ]]; then
  echo "Error: Extracted changelog section for ${VERSION} is empty." >&2
  exit 1
fi

printf '%s\n' "$SECTION_CONTENT"
