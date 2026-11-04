#!/usr/bin/env bash
# Offline only: cargo and gh are stubs; all Git objects stay in a temporary repo.
set -euo pipefail
source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
sandbox=$(mktemp -d "${TMPDIR:-/tmp}/bopomofo-release-tests.XXXXXX")
trap 'rm -rf -- "$sandbox"' EXIT
mkdir -p "$sandbox/bin" "$sandbox/home" "$sandbox/config" "$sandbox/repo"

# Ignore caller Git configuration, credential helpers, hooks, and signing rules.
git_test() {
  env -i PATH="$PATH" HOME="$sandbox/home" GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 \
    git -C "$sandbox/repo" -c user.name='Release Test' \
    -c user.email='release-test@example.invalid' -c commit.gpgsign=false "$@"
}
git_test -c init.defaultBranch=main init -q
tree=$(git_test mktree </dev/null)
commit=$(printf 'Temporary release source\n' | git_test commit-tree "$tree")
other_commit=$(printf 'Different remote source\n' | git_test commit-tree "$tree" -p "$commit")
git_test update-ref refs/heads/main "$commit"
git_test tag v0.1.0 "$commit"
git_test checkout -q --detach v0.1.0
[[ -z $(git_test remote) ]]

cat > "$sandbox/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$#" -eq 5 && "$1" == metadata && "$2" == --locked && "$3" == --no-deps && "$4" == --format-version && "$5" == 1 ]]
printf '%s\n' 'cargo metadata' >> "$TEST_CALLS"
case "$TEST_SCENARIO" in
  cargo-error) echo 'stub cargo metadata failure' >&2; exit 1 ;;
  invalid-metadata) printf '{invalid JSON\n' ;;
  mixed-versions) printf '{"packages":[{"version":"0.1.0"},{"version":"0.2.0"}]}\n' ;;
  empty-workspace) printf '{"packages":[]}\n' ;;
  prerelease-version) printf '{"packages":[{"version":"0.1.0-rc.1"},{"version":"0.1.0-rc.1"}]}\n' ;;
  *) printf '{"packages":[{"version":"0.1.0"},{"version":"0.1.0"}]}\n' ;;
esac
STUB

cat > "$sandbox/bin/gh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
# The test must never pass caller tokens to a subprocess.
[[ "$GH_TOKEN" == offline-test-token && -z ${GITHUB_TOKEN+x} && -z ${GH_ENTERPRISE_TOKEN+x} && -z ${GITHUB_ENTERPRISE_TOKEN+x} ]]
if [[ "$#" -eq 4 && "$1" == api && "$2" == "repos/$GH_REPO/commits/$RELEASE_TAG" && "$3" == --jq && "$4" == .sha ]]; then
  printf '%s\n' 'gh api commit' >> "$TEST_CALLS"
  case "$TEST_SCENARIO" in
    commit-api-error) echo 'stub commit API failure' >&2; exit 1 ;;
    moved-tag) printf '%s\n' "$TEST_OTHER_COMMIT" ;;
    empty-remote-commit) printf '\n' ;;
    *) printf '%s\n' "$TEST_COMMIT" ;;
  esac
elif [[ "$#" -eq 5 && "$1" == api && "$2" == --paginate && "$3" == "repos/$GH_REPO/releases?per_page=100" && "$4" == --jq && "$5" == '.[].tag_name' ]]; then
  printf '%s\n' 'gh api releases' >> "$TEST_CALLS"
  case "$TEST_SCENARIO" in
    duplicate-draft|duplicate-published) printf 'v0.0.9\n%s\nv0.2.0\n' "$RELEASE_TAG" ;;
    releases-api-error) echo 'stub releases API failure' >&2; exit 1 ;;
    partial-releases-api-error) printf 'v0.0.9\n'; echo 'stub paginated API failure' >&2; exit 1 ;;
    unrelated-releases) printf 'v0.1.0-rc.1\nv0.1.01\nv0.0.9\n' ;;
  esac
elif [[ "$1" == release && "$2" == create ]]; then
  printf '%s\n' 'gh release create' >> "$TEST_CALLS"
  # Exact argument matching rejects assets, replacement flags, and extra calls.
  expected=(release create "$RELEASE_TAG" --repo "$GH_REPO" --verify-tag
    --draft --prerelease --latest=false --title "bopomofo $RELEASE_TAG"
    --generate-notes --notes "Source-only development prerelease. No native IME binaries are included. Native desktop validation, production lexicon, and representative performance gates remain open. See the [validation register](https://github.com/$GH_REPO/blob/$TEST_COMMIT/docs/native-validation.md).")
  [[ "$#" -eq "${#expected[@]}" ]]
  for argument in "${expected[@]}"; do
    [[ "$1" == "$argument" ]]
    shift
  done
else
  echo 'Unexpected gh command; real gh is never used.' >&2
  exit 1
fi
STUB
chmod +x "$sandbox/bin/cargo" "$sandbox/bin/gh"

tests=0
run_case() {
  local name=$1 expected=$2 scenario=$3 diagnostic=$4
  local ref_type=${5:-tag} tag=${6:-v0.1.0} omitted=${7:-}
  local calls="$sandbox/calls" log="$sandbox/output" status
  # env -i removes ALL real tokens and caller Git/gh settings, not just GH_TOKEN.
  local -a test_env=(
    "PATH=$sandbox/bin:$PATH" "HOME=$sandbox/home" "XDG_CONFIG_HOME=$sandbox/config"
    "TMPDIR=$sandbox" GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0
    GH_TOKEN=offline-test-token "TEST_SCENARIO=$scenario" "TEST_CALLS=$calls"
    "TEST_COMMIT=$commit" "TEST_OTHER_COMMIT=$other_commit"
  )
  [[ "$omitted" == RELEASE_REF_TYPE ]] || test_env+=("RELEASE_REF_TYPE=$ref_type")
  [[ "$omitted" == RELEASE_TAG ]] || test_env+=("RELEASE_TAG=$tag")
  [[ "$omitted" == GH_REPO ]] || test_env+=(GH_REPO=example/bopomofo)
  : > "$calls"
  if (
    cd -- "$sandbox/repo"
    env -i "${test_env[@]}" bash -c 'bash "$1" && bash "$2"' release-test \
      "$source_root/.github/scripts/validate-release-tag.sh" \
      "$source_root/.github/scripts/create-source-draft.sh"
  ) > "$log" 2>&1; then
    status=0
  else
    status=$?
  fi
  if [[ "$expected" == pass ]]; then
    if [[ "$status" -ne 0 ]] || ! diff -u <(printf '%s\n' 'cargo metadata' 'gh api commit' 'gh api releases' 'gh release create') "$calls"; then
      printf 'FAIL: %s (status %s)\n' "$name" "$status" >&2
      exit 1
    fi
  elif [[ "$status" -eq 0 ]] || grep -Fxq 'gh release create' "$calls" || ! grep -Fq -- "$diagnostic" "$log"; then
    printf 'FAIL: %s (status %s; expected rejection with no create)\n' "$name" "$status" >&2
    exit 1
  fi
  tests=$((tests + 1))
  printf 'PASS: %s\n' "$name"
}

run_case 'source-only draft, exactly one create' pass success ''
run_case 'unrelated release tags do not block creation' pass unrelated-releases ''
run_case 'matching prerelease workspace version' pass prerelease-version '' tag v0.1.0-rc.1
run_case 'branch ref' fail success 'Run Release on an existing version tag, not a branch.' branch
run_case 'tag/version mismatch' fail success 'Release tag must match the workspace version: v0.1.0' tag v0.2.0
run_case 'missing v prefix' fail success 'Release tag must match the workspace version: v0.1.0' tag 0.1.0
run_case 'mixed workspace versions' fail mixed-versions 'workspace package versions differ'
run_case 'empty workspace metadata' fail empty-workspace 'workspace package versions differ'
run_case 'cargo metadata failure' fail cargo-error 'stub cargo metadata failure'
run_case 'invalid cargo metadata JSON' fail invalid-metadata 'parse error'
run_case 'remote tag moved' fail moved-tag 'The remote release tag no longer matches the checked commit.'
run_case 'empty remote commit' fail empty-remote-commit 'The remote release tag no longer matches the checked commit.'
run_case 'existing draft' fail duplicate-draft 'A release already exists for this tag.'
run_case 'existing published release' fail duplicate-published 'A release already exists for this tag.'
run_case 'commit API error' fail commit-api-error 'stub commit API failure'
run_case 'releases API error' fail releases-api-error 'stub releases API failure'
run_case 'paginated API error after partial output' fail partial-releases-api-error 'stub paginated API failure'
run_case 'required ref type missing' fail success RELEASE_REF_TYPE tag v0.1.0 RELEASE_REF_TYPE
run_case 'required tag missing' fail success RELEASE_TAG tag v0.1.0 RELEASE_TAG
run_case 'required repository missing' fail success GH_REPO tag v0.1.0 GH_REPO

[[ -z $(git_test remote) ]]
printf 'Release workflow: %s offline cases passed (temporary repo; stub cargo/gh; no network or remote writes).\n' "$tests"
