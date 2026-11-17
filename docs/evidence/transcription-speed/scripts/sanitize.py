"""Writes the evidence that can be published from the raw run files.

usage: python -I sanitize.py RAW_DIR OUT_DIR

RAW_DIR holds the raw files of every run (phone logs, thermal and cpuset
polls, timing-harness reports, cues read from the app's store). OUT_DIR gets:

- transcripts/NAME.json: every report or cue file the evidence scores, with
  its windows and cue times as they were, and each word replaced by a keyed
  hash. The words are normalized first exactly as wer.py normalizes them, and
  equal words get equal hashes, so wer.py gives the same numbers on these
  files as on the raw ones. The key is random and discarded:
  the lecture's text, which is not ours to publish, cannot be recovered by
  hashing a dictionary.
- observations/NAME.txt: only the app's own `I/vto` timing, created and ready
  lines of each phone log (other apps' lines are left out), the harness's
  timing lines, and the thermal and cpuset polls as recorded.
"""
import hashlib
import hmac
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from wer import cues  # noqa: E402

TRANSCRIPTS = [
    'reference-large', 'harness-baseline-cold', 'bg-base3-cues', 'tx-final-cues', 'tx-final3-cues',
    'pc-base-plan2', 'pc-retry-plan2', 'tune-forced-0', 'seg-forced-1000', 'seg-forced-2000',
    'seg-forced-3000', 'seg-forced-4000', 'rs-upgrade-killed', 'rs-upgrade-end', 'rs-new-killed',
    'rs-new-end', 'coldwarm',
]
LOGS = ['bg-base3', 'tx-final', 'tx-final3', 'rs-upgrade', 'rs-new']
POLLS = ['harness-baseline-thermal', 'bg-base3-thermal', 'tx-final-thermal', 'tx-final3-thermal',
         'coldwarm-thermal', 'tx-final-cpuset', 'tx-final3-cpuset']
HARNESS_LOGS = ['harness-baseline-cold']


def normalized(text):
    """The words of `text` as wer.words() compares them."""
    ws = re.sub(r"[^\w'\- ]+", ' ', text.lower()).replace('-', ' ').split()
    return [w.strip("'") for w in ws if w.strip("'")]


def main():
    raw, out = sys.argv[1:3]
    key = os.urandom(32)
    word = lambda w: hmac.new(key, w.encode(), hashlib.sha256).hexdigest()[:10]
    os.makedirs(os.path.join(out, 'transcripts'), exist_ok=True)
    os.makedirs(os.path.join(out, 'observations'), exist_ok=True)
    for name in TRANSCRIPTS:
        src = json.load(open(os.path.join(raw, name + '.json'), encoding='utf-8'))
        runs = []
        for i, run in enumerate(src.get('runs', [{}])):
            r = {k: run[k] for k in ('pass', 'warm', 'infer_ms') if k in run}
            if 'regions' in run:
                r['regions'] = [{k: g[k] for k in ('i', 'start_ms', 'end_ms', 'ms') if k in g} for g in run['regions']]
            if 'runs' in src:
                cs = [(c['start_ms'], c['end_ms'], c.get('text') or '') for c in run['cues']]
            else:
                cs = cues(os.path.join(raw, name + '.json'))
            r['cues'] = [{'start_ms': a, 'end_ms': b, 'text': ' '.join(word(w) for w in normalized(t))} for a, b, t in cs]
            runs.append(r)
        head = {k: src[k] for k in ('audio_ms', 'decode_ms', 'vad_ms', 'load_ms', 'plan', 'speech_ms', 'samples') if k in src}
        json.dump({**head, 'runs': runs}, open(os.path.join(out, 'transcripts', name + '.json'), 'w', encoding='utf-8'),
                  separators=(',', ':'))
    for name in LOGS:
        lines = open(os.path.join(raw, name + '.logcat'), encoding='utf-8', errors='replace').read().splitlines()
        keep = sorted({l for l in lines if 'I/vto' in l and re.search(r'timing job|created for|\bready$', l)})
        open(os.path.join(out, 'observations', name + '-timing.txt'), 'w', encoding='utf-8', newline='\n').write('\n'.join(keep) + '\n')
    for name in HARNESS_LOGS:
        lines = open(os.path.join(raw, name + '.log'), encoding='utf-8', errors='replace').read().splitlines()
        keep = [l for l in lines if l.startswith('timing ')]
        open(os.path.join(out, 'observations', name + '-timing.txt'), 'w', encoding='utf-8', newline='\n').write('\n'.join(keep) + '\n')
    for name in POLLS:
        # A sample taken while the phone was unplugged reads empty: its time is kept, without the trailing space.
        lines = open(os.path.join(raw, name + '.txt'), encoding='utf-8', errors='replace').read().splitlines()
        open(os.path.join(out, 'observations', name + '.txt'), 'w', encoding='utf-8', newline='\n').write(''.join(l.rstrip() + '\n' for l in lines))


if __name__ == '__main__':
    main()
