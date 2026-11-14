import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFileSync} from 'node:fs';

export async function fixture(root) {
  const media = readFileSync(`${root}/src-tauri/tests/fixtures/library-video.mp4`);
  const calls = [];
  const server = createServer((request, response) => {
    calls.push({method: request.method, path: request.url});
    if (!/^\/(?:saved|other|untranslated)\.mp4$/.test(request.url)) {
      response.writeHead(404); response.end(); return;
    }
    const range = /^bytes=(\d+)-(\d*)$/.exec(request.headers.range ?? '');
    const start = range ? Number(range[1]) : 0;
    const end = range?.[2] ? Math.min(Number(range[2]), media.length - 1) : media.length - 1;
    if (start > end || start >= media.length) {response.writeHead(416); response.end(); return;}
    response.writeHead(range ? 206 : 200, {'Content-Type': 'video/mp4', 'Accept-Ranges': 'bytes',
      'Content-Length': end - start + 1, 'Access-Control-Allow-Origin': '*',
      ...(range ? {'Content-Range': `bytes ${start}-${end}/${media.length}`} : {})});
    response.end(media.subarray(start, end + 1));
  });
  await new Promise((resolve, reject) => {server.once('error', reject); server.listen(8137, '127.0.0.1', resolve);});
  return {calls, assertNoTranslation() {assert.ok(calls.every(call => call.path.endsWith('.mp4')), 'Unexpected translation/model request');},
    async close() {server.closeAllConnections(); await new Promise(resolve => server.close(resolve));}};
}
