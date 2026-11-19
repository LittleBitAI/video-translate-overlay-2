"""python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py: checks that the
rendered-line tables in ../README.md are sums of expected.json, the counts measure.test.svelte.ts
asserts. Prints each row that differs and the number of differences (0 when they agree)."""
import json
from pathlib import Path

here = Path(__file__).parent
e = json.loads((here / 'expected.json').read_text(encoding='utf-8'))
doc = (here.parent / 'README.md').read_text(encoding='utf-8')
bad = 0

cols = [('overlay', 'windowed'), ('overlay', 'fullscreen'), ('overlay', 'phone'), ('watch', 'windowed'), ('watch', 'fullscreen'), ('watch', 'phone')]
for key in e['overlay']['phone']['new']:
    cells = [f"{len(e[s][z]['old'][key]['over'])} → {len(e[s][z]['new'][key]['over'])}" for s, z in cols]
    row = f"| `{key}` | " + ' | '.join(cells) + ' |'
    if row not in doc:
        print('per-video row differs:', row)
        bad += 1

names = {'overlay': 'Page overlay', 'watch': 'Watch screen'}
sizes = {'windowed': 'windowed PC 1280×800', 'fullscreen': 'fullscreen PC 1920×1080', 'phone': 'phone 384×854'}
for s in e:
    for z in e[s]:
        def cell(label):
            vs = e[s][z][label].values()
            one, two = (sum(v['lines'].get(n, 0) for v in vs) for n in ('1', '2'))
            more = sum(len(v['over']) for v in vs)
            top = max([n for v in vs for _, n in v['over']] or [0])
            return f'{one:,} / {two:,} / {more:,}' + (f' (up to {top})' if label == 'old' else '')
        row = f'| {names[s]} | {sizes[z]} | {cell("old")} | {cell("new")} |'
        if row not in doc:
            print('aggregate row differs:', row)
            bad += 1

for key, v in e['watch']['phone']['new'].items():
    for seq, _ in v['over']:
        if f'`{key}` ' not in doc or str(seq) not in doc:
            print('exception not listed:', key, seq)
            bad += 1
print('differences', bad)
