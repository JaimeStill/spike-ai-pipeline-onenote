#!/usr/bin/env bash
# currency reports every dependency, toolchain, and Action pin that trails its latest
# release, one line each, and exits non-zero when it reports any. It reads the
# network, GitHub's API through gh among it, and writes nothing. Run it through
# `mise run currency`.
#
# Each scan captures its command's output in a variable before reading it, so
# under set -e a scan command that fails fails currency instead of reporting
# nothing.
set -euo pipefail
cd "$MISE_PROJECT_ROOT"

stale=0
report() {
	echo "$1"
	stale=1
}

# Crates: updates within each requirement ("Updating"), and newer releases
# outside it ("Unchanged ... (available: ...)"). --dry-run leaves Cargo.lock
# as it is.
crates=$(cargo update --dry-run --verbose 2>&1 |
	sed -nE 's/^ *(Updating|Unchanged) +([^ ]+) +(v[^ ]+)( -> (v[^ ]+)| \(available: (v[^)]+)\))$/Cargo: \2 \3 -> \5\6/p')
while read -r line; do
	[ -n "$line" ] && report "$line"
done <<<"$crates"

# mise tools, Rust among them, pinned below their latest version.
outdated=$(mise outdated --bump --local --json |
	jq -r 'to_entries[] | "mise.toml: \(.key) \(.value.requested) -> \(.value.bump)"')
while read -r line; do
	[ -n "$line" ] && report "$line"
done <<<"$outdated"

# GitHub Actions pinned below the action's latest release. Each is pinned by its exact
# release tag, "uses: <action>@<tag>", which is compared with the latest.
uses=$(sed -nE 's/^[ -]*uses: *([^ @]+)@([^ ]+).*$/\1 \2/p' .github/workflows/*.yml | sort -u)
while read -r action pin; do
	[ -n "$action" ] || continue
	repo=$(echo "$action" | cut -d/ -f1,2)
	latest=$(gh api "repos/$repo/releases/latest" --jq .tag_name)
	[ "$pin" = "$latest" ] || report ".github/workflows: $action $pin -> $latest"
done <<<"$uses"

exit "$stale"
