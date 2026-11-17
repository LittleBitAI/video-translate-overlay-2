"""Launch an isolated preview with the host's owned-process helper; the stop file closes its tree."""
import hashlib
import json
import os
import sys
import time
from pathlib import Path

root = Path.cwd()
out = root / os.environ.get('LAYOUT_OUTPUT', 'artifacts/maximized-layout/review-1/windows')
out.mkdir(parents=True, exist_ok=True)
tool = Path(os.environ['WIKI_HOST_TOOL'])
sys.path.insert(0, str(tool))
from native_process import launched

profile = out / 'profile'
profile.mkdir(exist_ok=False)
host_sources = []
for path in [tool / 'native_process.py', tool / 'common/process.py']:
    target = out / (path.name + '.source')
    target.write_bytes(path.read_bytes())
    host_sources.append({'name': target.name, 'sha256': hashlib.sha256(target.read_bytes()).hexdigest()})
env = {**os.environ, 'WIKI_NATIVE_PROFILE': str(profile / 'data'),
       'WEBVIEW2_USER_DATA_FOLDER': str(profile / 'webview'),
       'WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS': '--remote-debugging-address=127.0.0.1 --remote-debugging-port=9247'}
with (out / 'app.log').open('wb') as log:
    with launched([str(root / 'target/debug/video-translate-overlay.exe')], root, env, 'WinSta0\\Default', log) as child:
        (out / 'launch.json').write_text(json.dumps({'pid': child.pid, 'profile': str(profile), 'port': 9247,
                                                  'hostSources': host_sources}), encoding='utf-8')
        print(f'Owned Windows app pid={child.pid}, CDP port=9247', flush=True)
        while not (out / 'stop').exists():
            if child.poll() is not None:
                raise RuntimeError('Owned application exited early')
            time.sleep(0.2)
print('Owned Windows process tree stopped', flush=True)
