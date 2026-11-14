"""Seed a pre-library database; never open a user's existing data directory."""

import hashlib
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
folder, host, port = Path(sys.argv[1]), sys.argv[2], int(sys.argv[3])
folder.mkdir(parents=True, exist_ok=False)
schema = subprocess.check_output(
    ["git", "show", "dc6fec7fd4a6:src-tauri/src/store.rs"], cwd=root, encoding="utf-8"
)
schema = re.search(r'const SCHEMA: &str = "(.*?)";', schema, re.S)[1]
with sqlite3.connect(folder / "app.sqlite3") as db:
    db.executescript(schema)
    for name, title in [("saved", "Saved attention"), ("other", "Unrelated systems")]:
        url = f"http://{host}:{port}/{name}.mp4"
        key = hashlib.sha256(url.encode()).hexdigest()
        db.execute("INSERT INTO video VALUES (?,?,?,?,?,?,?,?)", (
            key, url, "other", title, 0, "en", "fixture glossary", "2026-11-13"))
        db.execute("INSERT INTO job (id,video_key,stage,translation_model,cost_krw,notified_at,created_at,updated_at) "
                   "VALUES (?,?,?,?,?,?,?,?)", (f"fixture-{name}", key, "ready", "gemini-3.1-flash-lite", 12.5,
                                              "2026-11-13", "2026-11-13", "2026-11-13"))
        for seq, start, end, text in [(0, 0, 2000, "저장된 어텐션 자막입니다."), (1, 2000, 5000, "원래 시각을 유지합니다.")]:
            db.execute("INSERT INTO cue VALUES (?,?,?,?,?,?)", (
                key, seq, start, end, "saved transcript", text if name == "saved" else "다른 영상의 자막입니다."))
(folder / "audio").mkdir()
(folder / "models").mkdir()
for name in ("saved", "other"):
    (folder / "audio" / f"fixture-{name}.wav").write_bytes(b"per-video cleanup sentinel")
(folder / "models" / "shared-model").write_text("preserve shared model", encoding="utf-8")
(folder / "stand-in-key").write_text("AIza" + "v" * 35, encoding="utf-8")
(folder / "gemini_base_url.txt").write_text(f"http://{host}:{port}", encoding="utf-8")
