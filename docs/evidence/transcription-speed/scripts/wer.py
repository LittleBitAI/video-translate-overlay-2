"""Word error rate of a transcript against the reference, overall and near window cuts.

usage: python -I wer.py REFERENCE.json HYPOTHESIS.json [--around-s 4] [--cuts-from REPORT.json]

Both files are timing-harness reports (runs[0].cues with start_ms, end_ms, text)
or app subtitle exports (cues with text_src or text). Words are lowercased with
punctuation removed. Near-cut WER compares the words whose cue-interpolated time
falls within --around-s of a cut (a hypothesis window starting where
the one before ended or earlier), in reference and hypothesis alike;
--cuts-from takes the cuts from another report, to score two plans at the same places.
--aligned also attributes the errors of one whole-transcript alignment to the
cuts: an edit counts for a cut when its reference word's time is within 2 s,
so a word that moved across the window edge is not counted twice.
"""
import json
import re
import sys

import numpy as np


def cues(path):
    d = json.load(open(path, encoding='utf-8'))
    cs = d['runs'][0]['cues'] if 'runs' in d else d['cues']
    out = []
    for c in cs:
        text = c.get('text') or c.get('text_src') or c.get('source_text') or ''
        out.append((c['start_ms'], c['end_ms'], text))
    return out


def words(cs):
    """(time_ms, word) with each word's time interpolated across its cue."""
    out = []
    for a, b, t in cs:
        ws = [w for w in re.sub(r"[^\w'\- ]+", ' ', t.lower()).replace('-', ' ').split() if w.strip("'")]
        for i, w in enumerate(ws):
            out.append((a + (b - a) * (i + 0.5) / max(len(ws), 1), w.strip("'")))
    return out


def distance(ref, hyp):
    """Levenshtein distance over words, one numpy row per reference word."""
    vocab = {}
    r = np.array([vocab.setdefault(w, len(vocab)) for w in ref])
    h = np.array([vocab.setdefault(w, len(vocab)) for w in hyp])
    j = np.arange(len(h) + 1)
    row = j.copy()
    for x in r:
        diag = row[:-1] + (h != x)
        nxt = np.empty_like(row)
        nxt[0] = row[0] + 1
        nxt[1:] = np.minimum(row[1:] + 1, diag)
        row = np.minimum.accumulate(nxt - j) + j  # insertions along the row
    return int(row[-1])


def ops(ref, hyp):
    """Reference index of every edit of a minimal word alignment (insertions take the next index)."""
    vocab = {}
    r = np.array([vocab.setdefault(w, len(vocab)) for w in ref])
    h = np.array([vocab.setdefault(w, len(vocab)) for w in hyp])
    n, m = len(r), len(h)
    back = np.zeros((n + 1, m + 1), dtype=np.uint8)  # 0 match/sub, 1 deletion, 2 insertion
    back[0, 1:] = 2
    back[1:, 0] = 1
    j = np.arange(m + 1)
    row = j.copy()
    for i, x in enumerate(r, 1):
        diag = row[:-1] + (h != x)
        up = row[1:] + 1
        nxt = np.empty_like(row)
        nxt[0] = i
        nxt[1:] = np.minimum(up, diag)
        b = np.zeros(m + 1, dtype=np.uint8)
        b[0] = 1
        b[1:] = np.where(up < diag, 1, 0)
        best = np.minimum.accumulate(nxt - j) + j
        b[best < nxt] = 2
        back[i] = b
        row = best
    out, i, k = [], n, m
    while i or k:
        d = back[i, k]
        if d == 0:
            if r[i - 1] != h[k - 1]:
                out.append(i - 1)
            i, k = i - 1, k - 1
        elif d == 1:
            out.append(i - 1)
            i -= 1
        else:
            out.append(min(i, n - 1))
            k -= 1
    return out


def main():
    args = sys.argv[1:]
    around = float(args[args.index('--around-s') + 1]) * 1000 if '--around-s' in args else 4000
    ref, hyp = words(cues(args[0])), words(cues(args[1]))
    e = distance([w for _, w in ref], [w for _, w in hyp])
    print(f'overall: ref_words={len(ref)} hyp_words={len(hyp)} errors={e} wer={e / len(ref):.4f}')
    src = args[args.index('--cuts-from') + 1] if '--cuts-from' in args else args[1]
    rs = json.load(open(src, encoding='utf-8')).get('runs', [{}])[0].get('regions')
    if rs:
        # Every place one window ends and the next begins, where the two touch or overlap.
        cuts = [b['start_ms'] for a, b in zip(rs, rs[1:]) if b['start_ms'] - a['end_ms'] <= 0]
        er = n = 0
        for c in cuts:
            rw = [w for t, w in ref if abs(t - c) <= around]
            hw = [w for t, w in hyp if abs(t - c) <= around]
            er += distance(rw, hw) if rw else len(hw)
            n += len(rw)
        print(f'near {len(cuts)} cuts (+-{around / 1000:g} s): ref_words={n} errors={er} wer={er / max(n, 1):.4f}')
        if '--aligned' in args:
            t = [x for x, _ in ref]
            edits = [t[i] for i in ops([w for _, w in ref], [w for _, w in hyp])]
            near = lambda x: any(abs(x - c) <= 2000 for c in cuts)
            nw = sum(1 for x in t if near(x))
            ne = sum(1 for x in edits if near(x))
            print(f'aligned, +-2 s of {len(cuts)} cuts: ref_words={nw} errors={ne} wer={ne / max(nw, 1):.4f} (all edits {len(edits)})')


if __name__ == '__main__':
    main()
