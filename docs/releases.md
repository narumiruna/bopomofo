# Release workflow

## Scope

`.github/workflows/release.yml` creates a **source-only draft prerelease** for an existing `v<version>` tag. It does not publish the draft, upload binary packages, publish Cargo crates, or install/register/enable an IME.

Native desktop validation, production lexicon, representative performance, and distribution approval gates remain open. See the [validation register](native-validation.md) and [data sources](../data/SOURCES.md). Passing CI does not establish OS, CPU architecture, or application support.

## Checks and release notes

The workflow uses the tagged source for all jobs:

1. Require a tag whose name is `v` followed by the common version of all workspace packages. `Cargo.toml` owns the version under `[workspace.package]`.
2. Run the reusable [core and native build checks](../.github/workflows/checks.yml): Rust formatting, Clippy, workspace tests, Linux offline release and lightweight generator tests, macOS bundle/contracts, and Linux x86_64/ARM64 headless contracts.
3. Run the reusable [Windows checks](../.github/workflows/windows.yml): x64, x86, and ARM64 DLL builds and non-registration tests.
4. After both workflows pass, confirm that the remote tag still points to the checked commit and that no draft or published release uses that tag.
5. Create a draft marked as a prerelease, with GitHub-generated release notes and an explicit source-only warning. GitHub provides source archives for the tag; this workflow attaches no files.

Ordinary branch pushes and pull requests retain their existing CI checks. Version-tag pushes run those checks through Release instead of starting duplicate standalone workflows. Other tag names do not trigger these workflows.

Workflow permissions default to `contents: read`. Only draft creation has `contents: write`. It uses `GITHUB_TOKEN`; no separate release token or secret is required. Checkout does not retain Git credentials.

## Offline checks

Release uses the same [tag validation script](../.github/scripts/validate-release-tag.sh) and [draft creation script](../.github/scripts/create-source-draft.sh) as the [offline tests](../.github/tests/release-workflow.sh). Run these checks without credentials:

```bash
bash -n .github/scripts/validate-release-tag.sh
bash -n .github/scripts/create-source-draft.sh
bash -n .github/tests/release-workflow.sh
bash .github/tests/release-workflow.sh
```

The tests use a temporary Git repository with no remotes and stub `cargo` and `gh` commands. Each script runs with a clean environment and a fake token. Real tokens and caller Git/gh settings are not passed to the scripts. Successful cases require exactly one source-only create with `--draft --prerelease --latest=false --verify-tag`, generated notes, and no assets. Rejected refs, versions, moved tags, duplicate releases, API failures, and missing required variables must not attempt creation. The existing Linux CI job runs this suite.

Bash syntax checks, workflow YAML parsing with PyYAML 6.0.3, and actionlint 1.7.9 check local source only. The local actionlint run disables its optional ShellCheck and Pyflakes integrations. These checks do not prove hosted permissions, runner availability, or remote draft creation. Do not run the draft creation script against a real repository as a local test.

## Prepare a release

These commands are instructions, not evidence that a release was run. Obtain explicit, scoped authorization for tag push or workflow dispatch, hosted runner use, and source-only draft creation. Publication needs separate authorization. Record the approved repository, full commit SHA, common workspace version, and exact tag before any remote write. Do not put secrets in the approval record or logs.

1. Merge the release workflow into the default branch before the first release.
2. Review `docs/native-validation.md` and `data/SOURCES.md`. Keep this release source-only. Do not add native binaries or claim product readiness while those gates remain open.
3. Set the intended workspace version in `Cargo.toml` and update its workspace entries in `Cargo.lock`. Commit the reviewed version changes and source. Check that this commit matches the approved full SHA. `v0.1.0` below is an example for the current workspace version, not an approved release tag or commit.
4. After authorization, create an annotated tag on the approved commit and push only that tag. Replace `APPROVED_COMMIT_SHA` with the full approved SHA:

   ```bash
   git tag -a v0.1.0 APPROVED_COMMIT_SHA -m 'Release v0.1.0'
   git push origin refs/tags/v0.1.0
   ```

5. Inspect the Release run and its draft. Review the generated notes, source commit, prerelease marker, and validation warning. Generated notes are not validation evidence.
6. Leave the draft unpublished until the release owner obtains separate publication approval. Native binary distribution also requires the applicable native/data gates and signing/notarization decisions.

The workflow does not change versions or create tags. GitHub selects the comparison range for generated notes, including for the first release.

## Retry and recovery

- Failed tag validation or CI prevents draft creation. Correct source/version problems in a new commit and use a new version tag; do not move an existing release tag.
- After a transient runner or API failure, rerun the failed jobs for the same unchanged tag. An authorized operator can also dispatch Release for that tag:

  ```bash
  gh workflow run release.yml --ref v0.1.0
  ```

  Dispatch on a branch fails tag validation.
- If a draft or published release already exists, creation fails without replacing it. Inspect that release before any manual action. A retry after a successful creation also fails this check.
- If creation fails after GitHub accepts the request, inspect the Releases page before retrying. Preserve any reviewed draft text. Do not delete or replace a published release automatically.
- Disable the Release workflow in GitHub Actions to stop automation. This does not remove tags or releases.

## First hosted run handoff

**Hosted Release run: unverified. Remote draft creation and inspection: unverified.** Local stub tests and lint do not close either gate. This work did not push a tag, dispatch a workflow, create a remote release, or publish a draft. Installed native IME checks remain separate and unverified.

The release owner must obtain the scoped authorization above, run the approved hosted path, and retain this evidence. No commit, version, or tag is approved by this document.

| Field | Required evidence |
| --- | --- |
| Approval | Release owner, date, repository, full approved commit SHA, workspace version, exact tag, and authorized operations: tag push or dispatch, runner use, and source-only draft creation. Publication is excluded. |
| Source and tag | Local tag commit, remote tag commit, and workflow commit must equal the approved SHA. Record a commit URL and tag URL. Do not move a tag to make them match. |
| Hosted run | Actions run URL, attempt, event, ref, commit SHA, and overall result. Keep failed or blocked results. |
| Jobs | Results and job URLs for `validate-tag`; `checks` Rust jobs on Linux/macOS/Windows, including Linux offline release tests; `checks` Fcitx5 jobs on x86_64/ARM64; `windows` native-build jobs on x64/x86/ARM64; and `draft`. All required jobs must pass before draft creation. |
| Remote draft | Draft URL and release ID, exact tag, `draft: true`, `prerelease: true`, not latest, and no uploaded assets. GitHub source archives are not native binary packages. |
| Generated notes | Reviewed title, generated comparison range and changes, and the source-only warning. Record the reviewed text or its review URL. Generated notes are not test evidence. |
| Open gates | Link to the validation register at the checked commit. Native desktop validation, production lexicon, representative performance, native binary distribution, signing/notarization, and publication remain separate gates. |

For the approved tag, compare the local peeled commit with the remote commit returned by the same API used in the draft job. These are read-only checks; replace the example repository and tag before use:

```bash
git rev-parse 'v0.1.0^{commit}'
gh api repos/OWNER/REPO/commits/v0.1.0 --jq .sha
```

Compare both results with the approved SHA and the Actions run SHA. Record all results. The draft job repeats the remote comparison after CI. An API error or a changed tag must stop creation, not bypass the check.

The draft notes must retain this warning and a validation-register link at the checked commit:

> Source-only development prerelease. No native IME binaries are included. Native desktop validation, production lexicon, and representative performance gates remain open.

Keep the draft unpublished. If hosted access, runners, permissions, or authorization are unavailable, record the affected operation as blocked or not run. Do not replace that evidence with local lint results.
