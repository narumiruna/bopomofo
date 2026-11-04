#!/usr/bin/env bash
set -euo pipefail

commit=$(git rev-parse HEAD)
remote_commit=$(gh api "repos/$GH_REPO/commits/$RELEASE_TAG" --jq .sha)
if [[ "$remote_commit" != "$commit" ]]; then
  echo '::error::The remote release tag no longer matches the checked commit.'
  exit 1
fi
existing_tags=$(gh api --paginate "repos/$GH_REPO/releases?per_page=100" --jq '.[].tag_name')
if grep -Fxq -- "$RELEASE_TAG" <<< "$existing_tags"; then
  echo '::error::A release already exists for this tag. Review it instead of replacing it.'
  exit 1
fi
gh release create "$RELEASE_TAG" \
  --repo "$GH_REPO" \
  --verify-tag \
  --draft \
  --prerelease \
  --latest=false \
  --title "bopomofo $RELEASE_TAG" \
  --generate-notes \
  --notes "Source-only development prerelease. No native IME binaries are included. Native desktop validation, production lexicon, and representative performance gates remain open. See the [validation register](https://github.com/$GH_REPO/blob/$commit/docs/native-validation.md)."
