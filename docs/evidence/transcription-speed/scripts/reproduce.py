"""Recomputes the evidence's scored claims from the published files alone.

usage: python -I reproduce.py [DATA_DIR]   (default: ../data next to this script)

Prints each claim with the value the README states, and exits non-zero if any
differs: word error rates against the large reference, the overlap table,
the restart checks, the cold and warm passes, and the final run's stage times.
"""
import json
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from wer import cues, words, distance, ops  # noqa: E402

data = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'data')
T = lambda n: os.path.join(data, 'transcripts', n + '.json')
O = lambda n: os.path.join(data, 'observations', n + '.txt')
failed = []


def claim(name, got, want):
    ok = got == want
    print(f"{'ok  ' if ok else 'FAIL'} {name}: {got}" + ('' if ok else f' (README: {want})'))
    if not ok:
        failed.append(name)


def cuts(report):
    rs = json.load(open(T(report), encoding='utf-8'))['runs'][0]['regions']
    return [b['start_ms'] for a, b in zip(rs, rs[1:]) if b['start_ms'] - a['end_ms'] <= 0]


ref = words(cues(T('reference-large')))
rw = [w for _, w in ref]


def wer(name):
    hyp = [w for _, w in words(cues(T(name)))]
    return f'{distance(rw, hyp) / len(rw) * 100:.2f} %'


def near(name, at, around=4000):
    hyp = words(cues(T(name)))
    er = n = 0
    for c in at:
        r = [w for t, w in ref if abs(t - c) <= around]
        h = [w for t, w in hyp if abs(t - c) <= around]
        er += distance(r, h) if r else len(h)
        n += len(r)
    return f'{er / n * 100:.2f} %'


def aligned(name, at):
    t = [x for x, _ in ref]
    edits = [t[i] for i in ops(rw, [w for _, w in words(cues(T(name)))])]
    return sum(1 for x in edits if any(abs(x - c) <= 2000 for c in at))


old_cuts = cuts('harness-baseline-cold')
claim('plan 1 cuts through speech', len(old_cuts), 60)
for name, overall, at_cuts in [('harness-baseline-cold', '5.40 %', '13.43 %'), ('bg-base3-cues', '5.40 %', '13.43 %'),
                               ('pc-base-plan2', '5.85 %', '12.16 %'), ('pc-retry-plan2', '5.39 %', '12.16 %'),
                               ('tx-final-cues', '5.79 %', '11.97 %'), ('tx-final3-cues', '5.33 %', '11.97 %')]:
    claim(f'{name} overall', wer(name), overall)
    claim(f'{name} within 4 s of plan 1 cuts', near(name, old_cuts), at_cuts)
claim('background baseline cues equal the harness baseline cues',
      cues(T('bg-base3-cues')) == cues(T('harness-baseline-cold')), True)

forced = cuts('tune-forced-0')
claim('forced cuts', len(forced), 71)
for name, want in [('tune-forced-0', 41), ('seg-forced-1000', 85), ('seg-forced-2000', 51), ('seg-forced-3000', 41), ('seg-forced-4000', 40)]:
    claim(f'{name} errors within 2 s of the 71 cuts', aligned(name, forced), want)

ordered = lambda cs: all(a[0] < a[1] <= b[0] for a, b in zip(cs, cs[1:]))
final = cues(T('tx-final3-cues'))
claim('final cues', len(final), 856)
claim('final cues ordered and not overlapping', ordered(final), True)
k, e = cues(T('rs-upgrade-killed')), cues(T('rs-upgrade-end'))
claim('old job: cues at the kill unchanged after resuming', (len(k), e[:len(k)] == k, len(e), ordered(e)), (44, True, 93, True))
claim('old job: cues at the kill equal the baseline harness', cues(T('harness-baseline-cold'))[:len(k)] == k, True)
k, e = cues(T('rs-new-killed')), cues(T('rs-new-end'))
claim('new job: resumed cues equal the uninterrupted paid run', (len(k), e[:len(k)] == k, len(e), final[:len(e)] == e), (64, True, 118, True))
log = lambda n: open(O(n), encoding='utf-8').read()
claim('old job resumed on its own plan', bool(re.search(r'vad ms=\d+ plan=1 regions=190 speech_ms=\d+ from=9', log('rs-upgrade-timing'))), True)
claim('new job resumed on its own plan', bool(re.search(r'vad ms=\d+ plan=2 regions=140 speech_ms=\d+ from=9', log('rs-new-timing'))), True)

cw = json.load(open(T('coldwarm'), encoding='utf-8'))['runs']
claim('cold and warm inference ms', [(r['pass'], r['warm'], r['infer_ms']) for r in cw], [(1, False, 105207), (2, True, 119260)])
claim('cold and warm passes wrote the same cues', cw[0]['cues'] == cw[1]['cues'], True)

t3 = log('tx-final3-timing')
stage = lambda s: int(re.search(s + r' ms=(\d+)', t3).group(1))
claim('final writing wall ms', int(re.search(r'infer ms=\d+ audio_ms=\d+ wall_ms=(\d+)', t3).group(1)), 2322092)
claim('final translation ms, calls, in flight', re.search(r'translate ms=(\d+) cues=(\d+) calls=(\d+) in_flight=(\d+)', t3).groups(), ('132572', '856', '856', '8'))
claim('final transfer, decode, vad, model load ms', [stage(s) for s in ('transfer', 'decode', 'vad', 'model-load')], [11414, 4178, 48, 129])
win = [int(m) for m in re.findall(r'region i=\d+ .* ms=(\d+) segs', t3)]
claim('final windows, over 30 s, slowest ms', (len(win), sum(m > 30000 for m in win), max(win)), (140, 2, 71681))
th = [l for l in log('tx-final3-thermal').splitlines() if l.startswith('t=')]
claim('final thermal samples all status 0', (len(th), all(' status=0 ' in l for l in th)), (82, True))
cs = log('tx-final3-cpuset').split('\n')
claim('final cpuset samples in /moderate', (sum('/moderate' in l for l in cs), sum(bool(l.strip()) for l in cs)), (40, 41))
bl = log('bg-base3-timing')
claim('baseline background writing wall ms', int(re.search(r'infer ms=\d+ audio_ms=\d+ wall_ms=(\d+)', bl).group(1)), 3623989)
bw = [int(m) for m in re.findall(r'region i=\d+ .* ms=(\d+) segs', bl)]
claim('baseline background windows, over 30 s', (len(bw), sum(m > 30000 for m in bw)), (190, 11))
bt = [l for l in log('bg-base3-thermal').splitlines() if l.startswith('t=')]
claim('baseline background thermal samples, at status 0, highest status',
      (len(bt), sum(' status=0 ' in l for l in bt), max(int(re.search(r'status=(\d+)', l).group(1)) for l in bt)), (127, 123, 1))

sys.exit(1 if failed else 0)
