#!/usr/bin/env bash
set -Eeuo pipefail

REPO="${GITHUB_REPOSITORY:-gpkvt/cosmic-ext-fritzsmart-monitor-applet}"
GH=(gh)

usage() {
  cat <<'EOF'
Repository workflow helper

Usage:
  ./scripts/repo.sh update
      Fast-forward the current checkout from its configured upstream.

  ./scripts/repo.sh check
      Run Rust formatting, compilation, lint and test checks locally.

  ./scripts/repo.sh publish-pages
      Build, validate and publish docs/ to the gh-pages branch.

  ./scripts/repo.sh publish-release [patch|minor|major]
      Run the release pipeline: bump version, build packages, publish the
      GitHub release and trigger GitHub Pages publication.

  ./scripts/repo.sh build-release [ref]
      Build Debian, AppImage and Flatpak packages for a branch, tag or commit.
      The resulting packages are workflow artifacts (not a GitHub release).

  ./scripts/repo.sh workflows
      List repository workflows.

  ./scripts/repo.sh runs [workflow]
      List recent workflow runs, optionally filtered by workflow file/name.

  ./scripts/repo.sh watch RUN_ID
      Follow a workflow run until it finishes and print its result.

Environment:
  GITHUB_REPOSITORY  Override the repository (default: gpkvt/cosmic-ext-fritzsmart-monitor-applet)
EOF
}

die() { printf 'Error: %s\n' "$*" >&2; exit 1; }
require_gh() {
  command -v gh >/dev/null 2>&1 || die "GitHub CLI (gh) is required."
  gh auth status >/dev/null 2>&1 || die "Authenticate first with: gh auth login"
  "${GH[@]}" repo view "$REPO" --json nameWithOwner --jq .nameWithOwner >/dev/null ||
    die "Cannot access repository $REPO."
}
dispatch() {
  local workflow="$1"
  shift
  printf 'Dispatching %s in %s...\n' "$workflow" "$REPO"
  "${GH[@]}" workflow run "$workflow" --repo "$REPO" --ref main "$@"
  printf 'Workflow dispatched. Follow it with: make workflow-runs or make workflow-watch RUN_ID=<id>\n'
}

command="${1:-help}"
[[ $# -eq 0 ]] || shift

case "$command" in
  help|-h|--help)
    usage
    ;;
  update|pull)
    [[ $# -eq 0 ]] || die "update does not accept arguments."
    git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "Run this command inside a Git checkout."
    branch="$(git branch --show-current)"
    [[ -n "$branch" ]] || die "Detached HEAD: check out a branch before updating."
    printf 'Fast-forwarding %s from its configured upstream...\\n' "$branch"
    git pull --ff-only
    ;;
  check|checks)
    [[ $# -eq 0 ]] || die "check does not accept arguments."
    cargo fmt --all -- --check
    cargo check --locked --all-targets
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --all-targets
    ;;
  publish-pages|pages)
    [[ $# -eq 0 ]] || die "publish-pages does not accept arguments."
    require_gh
    dispatch gh-pages.yml
    ;;
  publish-release|release)
    bump="${1:-patch}"
    [[ $# -le 1 ]] || die "publish-release accepts at most one bump type."
    case "$bump" in patch|minor|major) ;; *) die "Bump type must be patch, minor or major." ;; esac
    require_gh
    dispatch release.yml -f "bump_type=$bump"
    ;;
  build-release|build-packages)
    ref="${1:-main}"
    [[ $# -le 1 ]] || die "build-release accepts at most one ref."
    [[ -n "$ref" ]] || die "Ref must not be empty."
    require_gh
    dispatch generate-linux-packages.yml -f "ref=$ref"
    ;;
  workflows)
    require_gh
    "${GH[@]}" workflow list --repo "$REPO"
    ;;
  runs|workflow-runs)
    require_gh
    if [[ $# -gt 1 ]]; then die "runs accepts at most one workflow filter."; fi
    if [[ $# -eq 1 ]]; then
      "${GH[@]}" run list --repo "$REPO" --workflow "$1" --limit 20
    else
      "${GH[@]}" run list --repo "$REPO" --limit 20
    fi
    ;;
  watch|workflow-watch)
    [[ $# -eq 1 && "$1" =~ ^[0-9]+$ ]] || die "Usage: repo.sh watch RUN_ID"
    require_gh
    "${GH[@]}" run watch "$1" --repo "$REPO" --exit-status
    ;;
  *)
    usage >&2
    die "Unknown command: $command"
    ;;
esac
