"""scripts/port/aliases.py [path...]: list `const a = b;` / `let a = b;` lines
in generated JS whose upstream file has no such `a ... = b`: copies rust-js
made (a gap to file, or a port-side form to list in the commit). Paths limit
the search to files under them (default: every module that has Rust).
Run from anywhere; UPSTREAM overrides upstream's commit (default 83b9c29).
"""
import os
import re
import subprocess
import sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
os.chdir(root)
upstream = os.environ.get('UPSTREAM', '83b9c29')
prefixes = sys.argv[1:] or ['packages/']
files = subprocess.run(['git', 'ls-files', 'packages/*.rs'], capture_output=True, text=True).stdout.split()
for rs in files:
    js = rs[:-3] + '.js'
    if not os.path.exists(js) or not any(js.startswith(p) for p in prefixes):
        continue
    up = subprocess.run(['git', 'show', f'{upstream}:{js}'], capture_output=True, text=True).stdout
    for i, line in enumerate(open(js), 1):
        m = re.match(r'\s*(?:const|let) ([\w$]+) = ([\w$]+);\s*$', line)
        if not m:
            continue
        a, b = m.groups()
        if b in ('null', 'true', 'false', 'undefined') or a.endswith('$1') or re.match(r'\d', b):
            continue
        if re.search(r'\b' + re.escape(a) + r'\b[^=\n]*=\s*' + re.escape(b) + r'\b', up):
            continue
        print(f'{js}:{i}: {line.strip()}')
