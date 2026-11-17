(async () => { window.confirm = () => true;
  const find = (k) => { let hit = null; const walk = (r) => { for (const e of r.querySelectorAll('*')) { if (e.shadowRoot) walk(e.shadowRoot); if (e.dataset?.vto === k) hit = hit ?? e; } }; walk(document); return hit; };
  const t0 = Date.now();
  find('detect').click();
  const seen = [];
  for (let i = 0; i < 40; i++) {
    await new Promise(r => setTimeout(r, 250));
    const s = find('status')?.textContent?.trim() ?? '';
    if (!seen.length || seen[seen.length - 1][1] !== s) seen.push([Date.now() - t0, s]);
    if (s.startsWith('음성 인식 모델')) break;
  }
  return { pressed_at: t0, seen };
})()
