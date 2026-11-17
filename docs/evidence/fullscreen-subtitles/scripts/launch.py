import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time

root = Path.cwd()
out = root / 'artifacts/subtitle-scaling/windows-final'
out.mkdir(parents=True, exist_ok=True)
profile = out / 'profile'
subprocess.run([sys.executable, 'scripts/verification/seed.py', str(profile / 'data'), '127.0.0.1', '8159'], check=True)
with sqlite3.connect(profile / 'data/app.sqlite3') as db:
    db.execute('UPDATE video SET duration_ms=3000')
    db.execute('UPDATE cue SET start_ms=0,end_ms=1200 WHERE seq=0')
    db.execute('UPDATE cue SET start_ms=1200,end_ms=3000,text_ko=? WHERE seq=1', (
        '이 강의에서는 트랜스포머의 어텐션 메커니즘이 긴 문맥에서 어떻게 계산량을 줄이는지, 그리고 그 과정에서 생기는 근사 오차를 어떻게 다루는지 차례대로 설명합니다.\n두 번째 줄도 영상 안에 표시합니다.',))
sys.path.insert(0, os.environ['WIKI_HOST_TOOL'])
from native_process import launched

env = {**os.environ, 'WIKI_NATIVE_PROFILE': str(profile / 'data'),
       'WEBVIEW2_USER_DATA_FOLDER': str(profile / 'webview'),
       'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS': '--remote-debugging-address=127.0.0.1 --remote-debugging-port=9251'}
with (out / 'app.log').open('wb') as log:
    with launched([str(root / 'target/debug/video-translate-overlay.exe')], root, env, 'WinSta0\\Default', log) as child:
        (out / 'launch.json').write_text(json.dumps({'pid': child.pid, 'port': 9251, 'profile': str(profile)}), encoding='utf-8')
        print(f'Owned Windows preview: pid={child.pid}, port=9251', flush=True)
        while not (out / 'stop').exists():
            if child.poll() is not None:
                raise RuntimeError('Preview exited')
            time.sleep(0.2)
