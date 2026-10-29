#!/bin/sh
# scripts/port/fndiff.sh <file.js> [function...]: compare a generated file with
# upstream's function by function. With no names, lists each top-level function
# with its count of differing lines (most first), and MISSING / EXTRA functions;
# with names, prints those functions' diffs. Path as for cmp.sh.
[ $# -ge 1 ] || { sed -n '2,6p' "$0"; exit 2; }
. "$(dirname "$0")/common.sh"
P=$(path "$1"); shift; B=$TMP/$(echo "$P" | tr / _)
prep "$P" "$B" || exit 1
python3 scripts/port/fndiff.py "$B.up.js" "$B.gen.js" "$@"
