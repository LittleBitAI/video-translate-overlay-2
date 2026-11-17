// Minimal CDP client: node cdp.mjs <port> <navigate-url|-> [js-expression]
// CDP_PAGE (optional) picks the first page whose URL contains it.
const [port, nav, expr] = process.argv.slice(2);
const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = pages.find(p => p.type === 'page' && p.url.includes(process.env.CDP_PAGE ?? ''));
const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0; const waits = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && waits.has(d.id)) { waits.get(d.id)(d); waits.delete(d.id); } };
await new Promise(r => ws.onopen = r);
const call = (method, params = {}) => new Promise(r => { const i = ++id; waits.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
if (nav && nav !== '-') { await call('Page.navigate', { url: nav }); await new Promise(r => setTimeout(r, 8000)); }
if (expr) {
  const r = await call('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true, timeout: 60000 });
  console.log(JSON.stringify(r.result?.result?.value ?? r.result ?? r, null, 0));
} else console.log(page.url);
ws.close(); process.exit(0);
