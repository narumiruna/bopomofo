#!/usr/bin/env bash
set -euo pipefail

if [[ "$RELEASE_REF_TYPE" != tag ]]; then
  echo '::error::Run Release on an existing version tag, not a branch.'
  exit 1
fi
version=$(cargo metadata --locked --no-deps --format-version 1 | jq -er '
  [.packages[].version] | unique |
  if length == 1 then .[0] else error("workspace package versions differ") end
')
if [[ "$RELEASE_TAG" != "v$version" ]]; then
  echo "::error::Release tag must match the workspace version: v$version"
  exit 1
fi
