"""fndiff.py <upstream.js> <generated.js> [function...]

Splits two prettier-formatted files into their top-level functions (a line
`function f(` or `export function f(` up to the next `}` at column 0) and
diffs them by name. With no names: one line per function, its count of
differing lines, most first, and MISSING (upstream only) / EXTRA (generated
only). With names: those functions' unified diffs. Run by fndiff.sh.
"""
import difflib
import re
import sys


def functions(path):
    out, name, body = {}, None, []
    for line in open(path).read().split('\n'):
        m = re.match(r'^(?:export )?(?:async )?function\*? ?(\w+)\(', line)
        if m:
            if name:
                out[name] = body
            name, body = m.group(1), [line]
            continue
        if name:
            body.append(line)
            if line == '}':
                out[name], name, body = body, None, []
    if name:
        out[name] = body
    return out


def unexported(body):
    return [re.sub(r'^export ', '', line) for line in body]


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    up, gen, names = functions(sys.argv[1]), functions(sys.argv[2]), sys.argv[3:]
    rows = []
    for k in up:
        if k not in gen:
            print('MISSING', k)
            continue
        d = list(difflib.unified_diff(unexported(up[k]), unexported(gen[k]), lineterm='', n=1))
        n = len([x for x in d if x[:1] in '+-' and x[:3] not in ('+++', '---')])
        rows.append((n, k, d))
    for k in gen:
        if k not in up:
            print('EXTRA', k)
    if names:
        for _, k, d in rows:
            if k in names:
                print('\n'.join(d))
    else:
        for n, k, _ in sorted(rows, reverse=True):
            print(n, k)


main()
