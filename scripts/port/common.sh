# Sourced by the port's scripts: the repository root, upstream's commit and
# the scratch directory, and `prep`, which writes a file's two sides.
#
# UPSTREAM is the commit holding react/react at ab0d927 (this repository's
# root commit, 83b9c29, whose packages/ are ab0d927's byte for byte).
# PORT_TMP is where compared files go (default target/port/cmp).
cd "$(dirname "$0")/../.." || exit 1
UPSTREAM=${UPSTREAM:-83b9c29}
TMP=${PORT_TMP:-target/port/cmp}
mkdir -p "$TMP"
BIN=node_modules/.bin

# prep <path from the repo root> <out prefix>: <out>.up.js is upstream's
# file, Flow types removed, and <out>.gen.js the working tree's; both
# formatted by prettier with the repository's config, so only code differs.
prep() {
  git show "$UPSTREAM:$1" > "$2.up.flow.js" || return 1
  $BIN/flow-remove-types --pretty "$2.up.flow.js" 2>/dev/null |
    $BIN/prettier --stdin-filepath x.js > "$2.up.js" || return 1
  $BIN/prettier --stdin-filepath x.js < "$1" > "$2.gen.js" || return 1
}

# The path a caller gives, under packages/ or from the root.
path() { case $1 in packages/*) echo "$1" ;; *) echo "packages/$1" ;; esac; }
