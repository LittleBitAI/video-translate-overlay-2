"""Read-only, bounded public evidence from the disposable MIT library copy."""
import hashlib
import json
import sqlite3
import sys
from pathlib import Path


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=True, separators=(',', ':')).encode()).hexdigest()


with sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + '?mode=ro', uri=True) as db:
    db.execute('BEGIN')
    key = 'QDX-1M5Nj7s'
    cues = db.execute('SELECT start_ms,end_ms,source_text,text_ko FROM cue WHERE video_key=? ORDER BY seq', (key,)).fetchall()
    versions = db.execute("SELECT identity,body FROM sync_record WHERE video_key=? AND identity LIKE 't/%' ORDER BY identity", (key,)).fetchall()
    result = {
        'cue_count': len(cues),
        'source_and_translation_sha256': digest(cues),
        'without_first_cue_sha256': digest(cues[1:]),
        'other_cues_sha256': digest(db.execute('SELECT * FROM cue WHERE video_key<>? ORDER BY video_key,seq', (key,)).fetchall()),
        'playback_sha256': digest(db.execute('SELECT * FROM playback ORDER BY video_key').fetchall()),
        'playlists_sha256': digest(db.execute('SELECT * FROM playlist ORDER BY id').fetchall()),
        'memberships_sha256': digest(db.execute('SELECT * FROM playlist_entry ORDER BY playlist_id,video_key').fetchall()),
        'versions': [{'identity': identity, 'body_sha256': hashlib.sha256(body.encode()).hexdigest()} for identity, body in versions],
    }
    print(json.dumps(result))
