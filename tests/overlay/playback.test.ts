import { expect, onTestFinished, test, vi } from 'vitest';
import { watchPlayback } from '../../src/overlay/playback';
import type { Reply, Request } from '../../src/overlay/index';

function player() {
  const video = document.createElement('video'); document.body.append(video);
  const state = {time:0, duration:60, paused:true}; const writes: number[] = [];
  Object.defineProperties(video, {
    currentTime:{get:() => state.time, set:(value:number) => {writes.push(value); state.time=value;}},
    duration:{get:() => state.duration}, paused:{get:() => state.paused},
  });
  onTestFinished(() => video.remove());
  const fire = (name: string) => video.dispatchEvent(new Event(name));
  return {video, state, writes, fire, seek(at:number) {state.time=at; fire('seeking'); fire('seeked');},
    play(at:number) {state.time=at; state.paused=false; fire('play');}, pause() {state.paused=true; fire('pause');}};
}
const saved = (position=30000): Reply => ({status:200, body:{video_key:'saved', position_ms:position, duration_ms:60000}});
function mount(video: HTMLVideoElement, request: Request, ad: () => boolean = () => false) {
  const watcher = watchPlayback(video, 'https://lectures.example.org/saved', request, ad);
  onTestFinished(() => watcher.stop()); return watcher;
}

test('resume reads once and does not overwrite a saved position when autoplay starts before the reply', async () => {
  const p = player(); let answer: ((reply:Reply) => void) | undefined;
  const puts: any[] = []; let reads = 0;
  mount(p.video, async (method, _path, body) => {
    if (method === 'GET') {reads++; return new Promise(resolve => {answer=resolve;});}
    puts.push(body); return {status:200, body:{}};
  });
  p.play(0); p.fire('timeupdate'); expect(puts).toHaveLength(0);
  answer!(saved());
  await vi.waitFor(() => expect(p.writes).toEqual([30]));
  p.fire('seeking'); p.fire('seeked'); p.fire('timeupdate');
  expect(puts).toHaveLength(0);
  p.state.time=35; p.pause();
  await vi.waitFor(() => expect(puts).toHaveLength(1));
  expect(puts[0]).toMatchObject({position_ms:35000, duration_ms:60000});
  p.fire('loadedmetadata'); document.dispatchEvent(new Event('visibilitychange'));
  expect(reads).toBe(1); expect(p.writes).toEqual([30]);
});

test('a user seek wins over a delayed saved position and queued writes keep the latest local time', async () => {
  const p = player(); let read: ((reply:Reply) => void) | undefined; let write: ((reply:Reply) => void) | undefined;
  const positions: number[] = [];
  mount(p.video, async (method, _path, body:any) => {
    if (method === 'GET') return new Promise(resolve => {read=resolve;});
    positions.push(body.position_ms);
    if (positions.length === 1) return new Promise(resolve => {write=resolve;});
    return {status:200, body:{}};
  });
  p.seek(20); await vi.waitFor(() => expect(write).toBeTruthy());
  p.seek(25); read!(saved(40000)); write!({status:200, body:{}});
  await vi.waitFor(() => expect(positions).toEqual([20000,25000]));
  expect(p.state.time).toBe(25); expect(p.writes).toEqual([]);
});

test('resume waits out an advertisement and never seeks again after the initial supported player becomes ready', async () => {
  const p = player(); let ad = true; let reads=0;
  p.state.duration=5;
  mount(p.video, async method => {if (method === 'GET') reads++; return saved();}, () => ad);
  await vi.waitFor(() => expect(reads).toBe(1)); expect(p.writes).toEqual([]);
  ad=false; p.state.duration=60; p.fire('loadedmetadata');
  await vi.waitFor(() => expect(p.writes).toEqual([30]));
  p.fire('seeked'); p.seek(45); p.fire('durationchange');
  expect(p.writes).toEqual([30]); expect(p.state.time).toBe(45); expect(reads).toBe(1);
});

test.each(['account', 'deletion', 'stop'])('a late saved position cannot survive %s retirement', async kind => {
  const p = player(); let answer: ((reply:Reply) => void) | undefined;
  const watcher = mount(p.video, async () => new Promise(resolve => {answer=resolve;}));
  if (kind === 'account') window.dispatchEvent(new Event('vto-account-changed'));
  else if (kind === 'deletion') window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail:'saved'}));
  else watcher.stop();
  answer!(saved());
  await new Promise(resolve => setTimeout(resolve, 30)); p.fire('loadedmetadata');
  expect(p.writes).toEqual([]);
});

test('a failed local write retries on a later player event without an unhandled rejection', async () => {
  const p = player(); const positions:number[]=[];
  mount(p.video, async (method, _path, body:any) => {
    if (method === 'GET') return {status:404, body:{}};
    positions.push(body.position_ms);
    if (positions.length === 1) throw Error('interrupted local IPC');
    return {status:200, body:{}};
  });
  p.seek(10); await vi.waitFor(() => expect(positions).toEqual([10000]));
  p.seek(12); await vi.waitFor(() => expect(positions).toEqual([10000,12000]));
  expect(p.writes).toEqual([]);
});
