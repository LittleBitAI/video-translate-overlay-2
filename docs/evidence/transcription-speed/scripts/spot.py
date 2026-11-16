"""Listening sheet: 10 old window cuts and 10 technical terms, with the reference,
the old build's and the new build's words around each place.

usage: python -I spot.py REFERENCE.json OLD.json NEW.json CUTS_REPORT.json > sheet.md
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wer import cues, words, distance

TERMS = ['transformer', 'parameters', 'gpu', 'tokens', 'llama', 'fine-tuning', 'rlhf',
         'jailbreak', 'injection', 'neural network', 'scaling laws', 'chatgpt', 'gpt-4',
         'operating system', 'reinforcement']


def around(ws, t, s=4000):
    return ' '.join(w for x, w in ws if abs(x - t) <= s)


def stamp(ms):
    s = int(ms // 1000)
    return f'{s // 60}:{s % 60:02d}'


def main():
    ref_c = cues(sys.argv[1])
    ref, old, new = (words(cues(p)) for p in sys.argv[1:4])
    rs = json.load(open(sys.argv[4], encoding='utf-8'))['runs'][0]['regions']
    cuts = [b['start_ms'] for a, b in zip(rs, rs[1:]) if b['start_ms'] - a['end_ms'] <= 0]
    places = [('cut', c) for c in cuts[3::6][:10]]
    used = set()
    for term in TERMS:
        if len([p for p in places if p[0] != 'cut']) == 10:
            break
        for a, b, t in ref_c:
            if term in t.lower() and all(abs(a - x) > 60000 for x in used):
                places.append((term, (a + b) / 2))
                used.add(a)
                break
    print('| # | Time | Place | Reference (large model) | Old build | New build | Old / new errors |')
    print('|---|---|---|---|---|---|---|')
    for i, (kind, t) in enumerate(sorted(places, key=lambda p: p[1]), 1):
        r, o, n = around(ref, t), around(old, t), around(new, t)
        eo, en = distance(r.split(), o.split()), distance(r.split(), n.split())
        link = f'[{stamp(t - 4000)}](https://www.youtube.com/watch?v=zjkBMFhNj_g&t={int(t // 1000) - 4}s)'
        print(f'| {i} | {link} | {kind} | {r} | {o} | {n} | {eo} / {en} |')


if __name__ == '__main__':
    main()
