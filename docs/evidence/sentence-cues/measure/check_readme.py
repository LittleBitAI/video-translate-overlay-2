"""python -I -X utf8 docs/evidence/sentence-cues/measure/check_readme.py [places.md]: checks that
the rendered-line tables in ../README.md are sums of expected.json, the counts measure.test.svelte.ts
asserts, and that its reading totals (the summary table, each video's place count and tally) are
counts of the verdicts in ../places.md, or in the places file given, and that every place judged
worse there is listed under Regressions. Prints each row that differs
and the number of differences (0 when they agree)."""
import json
import re
import sys
from collections import Counter
from pathlib import Path

here = Path(__file__).parent
e = json.loads((here / 'expected.json').read_text(encoding='utf-8'))
doc = (here.parent / 'README.md').read_text(encoding='utf-8')
places = Path(sys.argv[1]) if len(sys.argv) > 1 else here.parent / 'places.md'
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


def secs(t):
    s = 0
    for x in t.split(':'):
        s = s * 60 + int(x)
    return s


# The reading totals and the regressions are copies of places.md, the record of every verdict:
# count them again.
verdicts, worse, video = {}, [], None
for line in places.read_text(encoding='utf-8').splitlines():
    if line.startswith('### '):
        video = line.split()[1]
        verdicts[video] = Counter()
    elif video and line.startswith('| ') and not line.startswith('| At') and not line.startswith('|---'):
        cols = line.rstrip().rstrip('|').split(' | ')
        verdicts[video][cols[-2].strip()] += 1
        if cols[-2].strip() == 'worse':
            worse.append((video, cols[0].strip('| ')))
# A place in the README may be named by an old cue's time; places.md puts it at the new
# sentence nearest to that time within 10 s, so a listing within 10 s names the same place.
regressions = doc[doc.index('## Regressions'):doc.index('## Reproducing')]
listed = {}
for m in re.finditer(r'`([\w-]{11})`([^`\n]*)', regressions):
    for t in re.finditer(r'(\d+:\d\d)(?:–(\d+:\d\d))?', m[2]):
        listed.setdefault(m[1], []).append((secs(t[1]) - 10, secs(t[2] or t[1]) + 10))
for key, at in worse:
    if not any(a <= secs(at) <= b for a, b in listed.get(key, [])):
        print('worse place not listed under Regressions:', key, at)
        bad += 1
reading = doc[doc.index('## Reading old and new Korean'):doc.index('## Regressions')]
sections = dict(re.findall(r'^### (\S+) [^\n]*\n(.*?)(?=^### |\Z)', reading, re.S | re.M))
if set(sections) != set(verdicts):
    print('videos differ:', sorted(set(sections) ^ set(verdicts)))
    bad += 1
for key, n in verdicts.items():
    b, s_, w = n['better'], n['same'], n['worse']
    if sum(n.values()) != b + s_ + w:
        print(key, 'has verdicts other than better, same, worse:', dict(n))
        bad += 1
    row = f"| `{key}` | {b + s_ + w} | {b} | {s_} | {w} |"
    if row not in reading:
        print('summary row differs:', row)
        bad += 1
    text = sections.get(key, '')
    tallies = re.findall(r'Tally: better (\d+), same (\d+), worse (\d+)', text)
    if tallies != [(str(b), str(s_), str(w))]:
        print(key, 'tally differs:', tallies, 'counted', (b, s_, w))
        bad += 1
    counts = re.findall(r'([\d,]+) in all', text) or re.findall(r'I read ([\d,]+) places', text)
    if [c.replace(',', '') for c in counts[:1]] != [str(b + s_ + w)]:
        print(key, 'place count differs:', counts[:1], 'counted', b + s_ + w)
        bad += 1
total = sum(verdicts.values(), Counter())
row = f"| All {len(verdicts)} | {sum(total.values()):,} | {total['better']} | {total['same']} | {total['worse']} |"
if row not in reading:
    print('summary row differs:', row)
    bad += 1
print('differences', bad)
