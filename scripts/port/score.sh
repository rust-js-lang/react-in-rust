#!/bin/sh
# scripts/port/score.sh <file.js>...: for each generated file, the count of
# lines differing from upstream at HEAD and in the working tree, "old -> new",
# to see whether a change (a regeneration, a port edit) moved toward upstream.
[ $# -ge 1 ] || { sed -n '2,4p' "$0"; exit 2; }
. "$(dirname "$0")/common.sh"
for a in "$@"; do
  P=$(path "$a"); B=$TMP/$(echo "$P" | tr / _)
  prep "$P" "$B" || continue
  git show "HEAD:$P" 2>/dev/null | $BIN/prettier --stdin-filepath x.js > "$B.head.js" 2>/dev/null
  o=$(diff "$B.up.js" "$B.head.js" | grep -c '^[<>]'); n=$(diff "$B.up.js" "$B.gen.js" | grep -c '^[<>]')
  echo "$o -> $n  $P"
done
