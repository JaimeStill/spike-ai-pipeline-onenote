#!/usr/bin/env bash
# findings runs the CLI's finding for evidence 17 on this machine, never in CI and never in the
# check: the fields the CLI recovers per field kind against the public fixtures' hand-written
# expectations (tests/fixtures/sections.json, notebooks.json and content.json), and its time and
# peak memory, median of FINDINGS_RUNS (3), on every committed input and on three large sections,
# each with and without --media. Run it through `mise run findings`, which passes its arguments
# here:
#
#   mise run findings                     keeps nothing: the results go to
#                                         .state/findings/results.json, overwritten each run, and
#                                         the tables to the terminal
#   mise run findings -- --record [name]  records the run in docs/findings/onenote-cli/runs/
#                                         <name>/results.json, <name> by default <date>-<n>; an
#                                         existing run directory is never overwritten
#
# A name is letters, digits, '.', '_', and '-', and doesn't start with '.'. It needs:
# - the three large sections, which it fetches into .state/findings/large/ at their pinned
#   commits and refuses unless each SHA-256 is the pinned one (never committed): Interop-
#   TestSuites' LargeData.one (11 MB), and onenote.rs's Large Desktop.one (113 MB) and Large
#   OneDrive.one (161 MB), both Git LFS objects, through GitHub's LFS media URL;
# - cc, to build bench/rusage.c, the wrapper that measures each run.
set -euo pipefail
cd "$MISE_PROJECT_ROOT"

usage() {
	echo "usage: mise run findings [-- --record [name]]" >&2
	exit 2
}

runs_dir=docs/findings/onenote-cli/runs
state=.state/findings
record=""
case "$#" in
0) ;;
1 | 2)
	[[ $1 == --record ]] || usage
	if [[ $# -eq 2 ]]; then
		record=$2
		if [[ ! $record =~ ^[A-Za-z0-9_-][A-Za-z0-9._-]*$ ]]; then
			echo "findings: run name '$record' isn't letters, digits, '.', '_', and '-', not starting with '.'" >&2
			exit 2
		fi
	else
		# The first <date>-<n> that no run directory has.
		day=$(date +%F)
		n=1
		while [[ -e $runs_dir/$day-$n ]]; do
			n=$((n + 1))
		done
		record=$day-$n
	fi
	if [[ -e $runs_dir/$record ]]; then
		echo "findings: $runs_dir/$record exists; a recorded run is never overwritten" >&2
		exit 1
	fi
	;;
*) usage ;;
esac

# fetch <url> <file> <sha256>: the file, fetched unless it is there with that SHA-256, and
# refused unless it then has it.
fetch() {
	local url=$1 file=$2 sha256=$3
	if [[ ! -f $file ]] || ! echo "$sha256  $file" | sha256sum --check --status; then
		echo "findings: fetching $url" >&2
		curl --fail --silent --show-error --location --output "$file.part" "$url"
		mv "$file.part" "$file"
	fi
	if ! echo "$sha256  $file" | sha256sum --check --status; then
		echo "findings: $file's SHA-256 isn't $sha256" >&2
		rm -f "$file"
		exit 1
	fi
}

# The large sections, at the commits the public fixture sets are pinned at. bench/findings.rs
# lists the same names, sources and SHA-256s; an LFS object's SHA-256 is its pointer's oid.
interop=fe87ed3253de01804a2ae6e1d0015943da6023f8
onenote_rs=727389a674665dc07e0c31818cd4b54b64b9c684
large=$state/large
mkdir -p "$large"
fetch "https://raw.githubusercontent.com/OfficeDev/Interop-TestSuites/$interop/FileSyncandWOPI/Setup/SUT/LargeData.one" \
	"$large/LargeData.one" 034d91a640d655a522efda1d98de4ec4c952aaa02e5ad59f7c35537ccf9c2ba1
fetch "https://media.githubusercontent.com/media/msiemens/onenote.rs/$onenote_rs/crates/parser/tests/samples/Large%20Desktop.one" \
	"$large/Large Desktop.one" 0a568f02de8bdc4196dfad97ce2ecac04456efd9376c275ff67af412a4604668
fetch "https://media.githubusercontent.com/media/msiemens/onenote.rs/$onenote_rs/crates/parser/tests/samples/Large%20OneDrive.one" \
	"$large/Large OneDrive.one" 713980fd942180f64910b71fc6236cd22f407facc9b8bd25e99b2eeb1ac9651a

# The release build the check makes, and the scorer and the wrapper.
target="$(uname -m)-unknown-linux-musl"
cargo build --release --locked --target "$target"
cargo build --release --locked --example findings
cc -O2 -Wall -Werror -o "$state/rusage" bench/rusage.c

commit=$(git describe --always --dirty)
"target/release/examples/findings" \
	--bin "target/$target/release/spike-ai-pipeline-onenote" \
	--rusage "$state/rusage" \
	--large "$large" \
	--state "$state" \
	--out "$state/results.json" \
	--commit "$commit" \
	--runs "${FINDINGS_RUNS:-3}"

if [[ -n $record ]]; then
	mkdir -p "$runs_dir/$record"
	cp "$state/results.json" "$runs_dir/$record/results.json"
	echo "findings: recorded $runs_dir/$record/results.json" >&2
fi
