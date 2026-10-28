# Release workflow

## Scope

`.github/workflows/release.yml` creates a **source-only draft prerelease** for an existing `v<version>` tag. It does not publish the draft, upload binary packages, publish Cargo crates, or install/register/enable an IME.

Native desktop validation, production lexicon, representative performance, and distribution approval gates remain open. See the [validation register](native-validation.md) and [data sources](../data/SOURCES.md). Passing CI does not establish OS, CPU architecture, or application support.

## Checks and release notes

The workflow uses the tagged source for all jobs:

1. Require a tag whose name is `v` followed by the common version of all workspace packages. `Cargo.toml` owns the version under `[workspace.package]`.
2. Run the reusable [core and native build checks](../.github/workflows/checks.yml): Rust formatting, Clippy, workspace tests, macOS bundle/contracts, and Linux x86_64/ARM64 headless contracts.
3. Run the reusable [Windows checks](../.github/workflows/windows.yml): x64, x86, and ARM64 DLL builds and non-registration tests.
4. After both workflows pass, confirm that the remote tag still points to the checked commit and that no draft or published release uses that tag.
5. Create a draft marked as a prerelease, with GitHub-generated release notes and an explicit source-only warning. GitHub provides source archives for the tag; this workflow attaches no files.

Ordinary branch pushes and pull requests retain their existing CI checks. Version-tag pushes run those checks through Release instead of starting duplicate standalone workflows. Other tag names do not trigger these workflows.

Workflow permissions default to `contents: read`. Only draft creation has `contents: write`. It uses `GITHUB_TOKEN`; no separate release token or secret is required. Checkout does not retain Git credentials.

## Prepare a release

These commands are instructions, not evidence that a release was run. Obtain explicit authorization before pushing a release tag, dispatching the release workflow, or publishing a draft.

1. Merge the release workflow into the default branch before the first release.
2. Review `docs/native-validation.md` and `data/SOURCES.md`. Keep this release source-only. Do not add native binaries or claim product readiness while those gates remain open.
3. Set the intended workspace version in `Cargo.toml` and update its workspace entries in `Cargo.lock`. Commit the reviewed version changes and source. For the current version, the tag is `v0.1.0`.
4. After authorization, create an annotated tag on the intended commit and push that tag:

   ```bash
   git tag -a v0.1.0 -m 'Release v0.1.0'
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

## Validation handoff

Local workflow validation and non-registration tests do not prove the hosted release path. The release owner must authorize the first GitHub run and check runner availability, repository permissions, generated notes, and draft creation. No release has been dispatched or published as part of adding this workflow. Installed native IME checks remain separate and unverified.
