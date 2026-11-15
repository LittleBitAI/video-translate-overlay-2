import type { Request } from './index';

const INTERVAL_MS = 5000;
const MAX_MS = 6 * 60 * 60 * 1000;

/** Read resume once per player. New input always wins over a delayed saved position. */
export function watchPlayback(video: HTMLVideoElement, pageUrl: string, request: Request, isAd: () => boolean) {
  let stopped = false;
  let retired = false;
  let controlled = video.currentTime > 1;
  let played = !video.paused;
  let moved = false;
  let readStarted = false;
  let readFinished = false;
  let pending: { position: number; duration: number } | null = null;
  let writing = false;
  let queued = false;
  let lastSent: number | undefined;
  let next = 0;
  let key: string | undefined;
  let resumeSeek: number | null = null;
  const deleted = new Set<string>();

  function tryResume() {
    if (stopped || retired || controlled || !pending || isAd() || document.visibilityState === 'hidden') return;
    const duration = video.duration * 1000;
    if (!Number.isFinite(duration) || duration <= 0 || pending.position > duration
      || (pending.duration > 0 && Math.abs(pending.duration - duration) > 1000)) return;
    const position = pending.position;
    pending = null;
    if (position <= 0) return;
    try {
      resumeSeek = position;
      lastSent = position;
      video.currentTime = position / 1000;
    } catch { resumeSeek = null; }
  }

  async function read() {
    if (readStarted || stopped || retired || document.visibilityState === 'hidden') return;
    readStarted = true;
    try {
      const result = await request('GET', '/playback', {page_url:pageUrl});
      if (stopped || retired || controlled) return;
      if (result.status !== 200) return;
      key = result.body.video_key;
      if (key && deleted.has(key)) return;
      const position = result.body.position_ms;
      const duration = result.body.duration_ms;
      if (!Number.isSafeInteger(position) || position < 0 || position > MAX_MS || !Number.isSafeInteger(duration) || duration < 0 || duration > MAX_MS) return;
      pending = {position, duration}; tryResume();
    } catch { /* A later write still records actual local playback when the core returns. */ }
    finally { readFinished = true; }
  }

  async function save(force = false) {
    if (stopped || retired || isAd() || (!played && !moved) || (!readFinished && !controlled)) return;
    const position = Math.round(video.currentTime * 1000);
    const duration = Math.round(video.duration * 1000);
    if (!Number.isSafeInteger(position) || !Number.isSafeInteger(duration) || duration <= 0 || duration > MAX_MS || position < 0 || position > duration) return;
    if (lastSent !== undefined && Math.abs(lastSent - position) < 500) return;
    if (writing) { queued = true; return; }
    if (!force && performance.now() < next) return;
    writing = true;
    next = performance.now() + INTERVAL_MS;
    try {
      const result = await request('PUT', '/playback', {page_url:pageUrl, position_ms:position, duration_ms:duration});
      if (!stopped && !retired && result.status === 200) lastSent = position;
    } catch { /* Retry from actual player time on its next event. */ }
    finally {
      writing = false;
      if (queued) { queued = false; void save(true); }
    }
  }

  function input(event: Event) {
    if (event.type === 'keydown' || (event.target instanceof Node && (video.closest('.html5-video-player') ?? video.parentElement)?.contains(event.target))) {
      controlled = true; pending = null; resumeSeek = null;
    }
  }
  function seek() {
    if (resumeSeek !== null && Math.abs(video.currentTime * 1000 - resumeSeek) < 250) {
      resumeSeek = null; return;
    }
    controlled = true; moved = true; pending = null; void save(true);
  }
  function seeking() {
    if (resumeSeek !== null && Math.abs(video.currentTime * 1000 - resumeSeek) < 250) return;
    controlled = true; moved = true; pending = null; resumeSeek = null;
  }
  function play() { played = true; void save(); }
  function tick() { void save(); }
  function pause() { void save(true); }
  function visible() {
    if (document.visibilityState === 'hidden') void save(true);
    else { void read(); tryResume(); }
  }
  function accountChanged() { retired = true; pending = null; }
  function invalidated(event: Event) {
    const changed = (event as CustomEvent).detail;
    if (typeof changed !== 'string') return;
    deleted.add(changed);
    if (key === changed) { controlled = true; pending = null; resumeSeek = null; lastSent = undefined; }
  }
  const events: [string, EventListener][] = [
    ['loadedmetadata', tryResume], ['durationchange', tryResume], ['timeupdate', tick],
    ['play', play], ['pause', pause], ['seeking', seeking], ['seeked', seek], ['ended', pause],
  ];
  for (const [name, callback] of events) video.addEventListener(name, callback);
  document.addEventListener('pointerdown', input, true);
  document.addEventListener('keydown', input, true);
  document.addEventListener('visibilitychange', visible);
  window.addEventListener('vto-account-changed', accountChanged);
  window.addEventListener('vto-video-deleted', invalidated);
  void read();
  return { stop() {
    stopped = true; pending = null;
    for (const [name, callback] of events) video.removeEventListener(name, callback);
    document.removeEventListener('pointerdown', input, true);
    document.removeEventListener('keydown', input, true);
    document.removeEventListener('visibilitychange', visible);
    window.removeEventListener('vto-account-changed', accountChanged);
    window.removeEventListener('vto-video-deleted', invalidated);
  } };
}
