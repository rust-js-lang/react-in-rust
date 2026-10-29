#!/bin/sh
# scripts/port/cmp.sh <file.js>: diff upstream's file (ab0d927, Flow types
# removed) with the generated one, both through prettier. The path is from the
# repo root or under packages/, e.g. react-dom-bindings/src/client/ReactDOMInput.js.
# The two sides stay in $PORT_TMP (target/port/cmp) for fndiff.sh.
[ $# -eq 1 ] || { sed -n '2,5p' "$0"; exit 2; }
. "$(dirname "$0")/common.sh"
P=$(path "$1"); B=$TMP/$(echo "$P" | tr / _)
prep "$P" "$B" || exit 1
diff "$B.up.js" "$B.gen.js"
