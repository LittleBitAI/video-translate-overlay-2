"""Writes one video's source cues from a copy of the app's SQLite store as {"cues": [...]}.

usage: python -I cues-from-db.py APP_SQLITE3 VIDEO_KEY OUT.json
"""
import json
import sqlite3
import sys

db, key, out = sys.argv[1:4]
c = sqlite3.connect(db)
rows = c.execute('SELECT seq, start_ms, end_ms, source_text FROM cue WHERE video_key = ? ORDER BY seq', (key,)).fetchall()
json.dump({'cues': [{'seq': s, 'start_ms': a, 'end_ms': b, 'text': t} for s, a, b, t in rows]},
          open(out, 'w', encoding='utf-8'), ensure_ascii=False, indent=1)
print(f'{len(rows)} cues, {rows[0][1] if rows else 0}-{rows[-1][2] if rows else 0} ms')
