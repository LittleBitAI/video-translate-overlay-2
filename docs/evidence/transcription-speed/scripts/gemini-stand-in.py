"""A loopback stand-in for Gemini, for running the debug desktop app without billing.

usage: python -I gemini-stand-in.py PORT LOG [DELAY_S]

The debug app reads its Gemini address from gemini_base_url.txt in its data
folder. This answers the key check (a model lookup) and every generateContent:
a glossary order gets one term, a translate order gets each segment back
prefixed with a Korean marker. Each reply waits DELAY_S (default 1.2 s, about
Gemini's own reply time on the phone runs), so requests overlap as they would.
LOG gets one JSON line per request: when it came and went, its task, how many
requests were open at once, and the output cap the app asked for.
"""
import json
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

port, log_path = int(sys.argv[1]), sys.argv[2]
delay = float(sys.argv[3]) if len(sys.argv) > 3 else 1.2
lock = threading.Lock()
open_now = 0
log = open(log_path, 'a', encoding='utf-8', newline='\n')


class Handler(BaseHTTPRequestHandler):
    def send(self, body):
        data = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.send({'name': self.path.rsplit('/', 1)[-1]})

    def do_POST(self):
        global open_now
        came = time.time()
        req = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        order = json.loads(req['contents'][0]['parts'][0]['text'])
        with lock:
            open_now += 1
            at_once = open_now
        time.sleep(delay)
        if order.get('task') == 'glossary':
            reply = {'terms': [{'source': 'transformer', 'target': '트랜스포머'}]}
        else:
            reply = {'segments': [{'id': s['id'], 'ko': '[한] ' + s['text']} for s in order['segments']]}
        text = json.dumps(reply, ensure_ascii=False)
        self.send({'candidates': [{'content': {'parts': [{'text': text}]}}],
                   'usageMetadata': {'promptTokenCount': len(req['contents'][0]['parts'][0]['text']) // 4 + 300,
                                     'candidatesTokenCount': len(text) // 4}})
        with lock:
            open_now -= 1
            log.write(json.dumps({'came': round(came, 3), 'went': round(time.time(), 3), 'task': order.get('task'),
                                  'segments': len(order.get('segments', [])), 'open_at_once': at_once,
                                  'max_output_tokens': req['generationConfig'].get('maxOutputTokens')}) + '\n')
            log.flush()

    def log_message(self, *_):
        pass


ThreadingHTTPServer(('127.0.0.1', port), Handler).serve_forever()
