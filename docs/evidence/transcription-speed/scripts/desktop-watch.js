// Evaluated in the desktop app's site window (cdp.mjs). Presses the overlay's
// detect button unless window.__vtoPress === false (a page reopened after a
// restart), then keeps window.__vtoRun up to date in the page: every change of
// the overlay's status text, and every change of the job's stage seen through
// the app's own bridge (get_job), until the job is ready or failed. At ready it
// adds what get_subtitles returns: the cue count, the first and last times,
// whether the cues are ordered without overlap, and how many carry Korean.
// Read it with: node cdp.mjs PORT - "JSON.stringify(window.__vtoRun)"
(() => {
  if (window.__vtoRun && !window.__vtoRun.done) return 'already running';
  const find = (k) => { let hit = null; const walk = (r) => { for (const e of r.querySelectorAll('*')) { if (e.shadowRoot) walk(e.shadowRoot); if (e.dataset?.vto === k) hit = hit ?? e; } }; walk(document); return hit; };
  const call = window.__TAURI_INTERNALS__.invoke;
  const page_url = location.href;
  const t0 = Date.now();
  const run = window.__vtoRun = { page_url, started_at: new Date(t0).toISOString(), started_at_ms: t0, pressed: window.__vtoPress !== false, status: [], stages: [] };
  // Library commands carry the account generation; as in the overlay, get_job's
  // refusal without one names it.
  let gen;
  const ask = async (cmd, args) => {
    try { return await call(cmd, { ...args, account_generation: gen }); } catch (e) {
      if (Number.isSafeInteger(e?.account_generation) && e.account_generation !== gen) { gen = e.account_generation; return ask(cmd, args); }
      return { error: e?.error ?? String(e) };
    }
  };
  window.confirm = () => true;
  if (run.pressed) find('detect').click();
  (async () => {
    let was = '';
    for (let i = 0; window.__vtoRun === run; i++) {
      await new Promise((r) => setTimeout(r, 250));
      const s = find('status')?.textContent?.trim() ?? '';
      if (!run.status.length || run.status[run.status.length - 1][1] !== s) run.status.push([Date.now() - t0, s]);
      if (i % 4) continue;
      const j = await ask('get_job', { page_url });
      const now = `${j.stage}|${j.preparing_model}|${j.failure_reason ?? ''}|${j.error ?? ''}`;
      if (now !== was) { run.stages.push({ ms: Date.now() - t0, stage: j.stage, percent: j.percent, preparing_model: j.preparing_model, failure_reason: j.failure_reason, error: j.error }); was = now; }
      run.last = { ms: Date.now() - t0, stage: j.stage, percent: j.percent, cost_krw: j.cost_krw };
      if (j.stage === 'ready' || j.stage === 'failed') {
        if (j.stage === 'ready') {
          const subs = await ask('get_subtitles', { video_key: j.video_key });
          const cues = subs.cues ?? [];
          run.subtitles = {
            cues: cues.length,
            first_ms: cues[0]?.start_ms, last_ms: cues[cues.length - 1]?.end_ms,
            ordered_without_overlap: cues.every((c, k) => c.start_ms < c.end_ms && (k === 0 || cues[k - 1].end_ms <= c.start_ms)),
            korean: cues.filter((c) => /[가-힣]/.test(c.text_ko ?? c.ko ?? c.text ?? '')).length,
          };
        }
        run.done = { ms: Date.now() - t0, stage: j.stage };
        return;
      }
    }
  })();
  return run.pressed ? 'pressed' : 'watching';
})()
