# Releasing RushDB

Release only from a signed, reviewed `vX.Y.Z` tag after CI passes.

## One-time setup

| What | Used by | Purpose |
|---|---|---|
| Repository `rush-db/homebrew-tap` | `publish.yml` | Holds `Formula/rushdb.rb` (`brew install rush-db/tap/rushdb`) |
| Secret `HOMEBREW_TAP_TOKEN` | `publish.yml` | Fine-grained token with contents write on `rush-db/homebrew-tap` |
| Secrets `DOCKERHUB_USERNAME`, `DOCKERHUB_TOKEN` | `publish.yml` | Push `rushdb/rushdb` to Docker Hub |
| Secret `CARGO_REGISTRY_TOKEN` | `release.yml` (manual run) | Publish the crates |
| GHCR package `rushdb` visibility: public | after the first push | Anonymous `docker pull ghcr.io/rush-db/rushdb` |
| GHCR package `charts/rushdb` visibility: public | after the first push | Anonymous `helm install oci://ghcr.io/rush-db/charts/rushdb` |

The Homebrew and Docker Hub steps are skipped while their secrets are unset;
GHCR needs only the workflow token.

## Steps

1. Set `[workspace.package] version`, move the `CHANGELOG.md` entry from
   *Unreleased* to the version and date, and run the quality gate from the
   README.
2. Push the tag. `release.yml`:
   - builds `rushdb` for Linux x86-64 and arm64 (inside
     `manylinux_2_28`, so they need glibc 2.28+) and macOS Apple silicon and
     Intel (deployment target 11.0);
   - packages each as `rushdb-<target>.tar.gz` with a `.sha256`, an SPDX
     SBOM, and a Protocol v1 capability snapshot, and signs all of them with
     Sigstore (GitHub OIDC, keyless);
   - verifies each archive with `scripts/verify-release-artifact.sh`
     (checksum, contents, startup, a durable write and read across a restart);
   - runs `scripts/test-install.sh`: the one-line installer on clean Debian,
     Ubuntu, Rocky Linux and Amazon Linux containers, plus the expected
     refusals on Alpine (musl) and CentOS 7 (glibc 2.17);
   - creates a draft GitHub release with every file and a copy of
     `install.sh`, marked as the latest release.
3. Review the draft (notes, capability snapshot) and publish it. Publishing
   triggers `publish.yml`, which uses the release's verified Linux binaries
   (nothing is rebuilt):
   - builds the `rushdb/rushdb` image for `linux/amd64` and `linux/arm64`,
     smoke-tests it (health check, a write, the refusal to start without a
     token, the arm64 binaries under emulation), pushes `X.Y.Z`, `X.Y` and
     `X` tags to `ghcr.io/rush-db/rushdb` and `rushdb/rushdb` with an SBOM
     and provenance, and signs the image with Sigstore;
   - packages `charts/rushdb` with the release version as both the chart
     version and `appVersion`, and pushes it to `oci://ghcr.io/rush-db/charts`;
   - regenerates `Formula/rushdb.rb` in the tap with
     `scripts/homebrew-formula.sh`.
4. Publish the crates: run `release.yml` manually with the tag and
   `publish: true`. `scripts/publish-crates.sh` packages every crate, then
   publishes them in dependency order, ending with `rushdb-server`. Only
   `rushdb-server` is meant for users (`cargo install` / `cargo binstall`); the
   others are internal components and say so in their descriptions.

`publish.yml` can be re-run for a published tag with `workflow_dispatch`.

The one-line installer is served from the default branch
(`https://raw.githubusercontent.com/rush-db/rushdb/main/install.sh`) and
installs the newest `vX.Y.Z` release, so a release is installable as soon as it
is published. Keep `install.sh` on the default branch compatible with the
archive names of every release users may pin.

## Verifying artifacts

Release archive:

```sh
cosign verify-blob \
  --bundle rushdb-x86_64-unknown-linux-gnu.tar.gz.sigstore.json \
  --certificate-identity-regexp '^https://github.com/rush-db/rushdb/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  rushdb-x86_64-unknown-linux-gnu.tar.gz
```

Container image:

```sh
cosign verify ghcr.io/rush-db/rushdb:3 \
  --certificate-identity-regexp '^https://github.com/rush-db/rushdb/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Dependency sources

A published crate resolves dependencies only from a registry; `path`, `git`,
and `[patch.*]` entries do not travel with the package. Run
`./scripts/check-package-sources.sh` before publishing: it fails on any such
source in a publishable crate.
