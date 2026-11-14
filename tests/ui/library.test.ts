import { mount, unmount } from 'svelte';
import { expect, onTestFinished, test, vi } from 'vitest';
import Library from '../../src/ui/Library.svelte';
import { $, api, button, calls, open, text, type } from './fake';

const video = {video_key:'saved',title:'Attention lecture',page_url:'https://lectures.example.org/a',saved:true,active:false,deleting:false};

test.each(['native', 'broadcast'].flatMap(source => [
  {operation:'rename', status:200}, {operation:'rename', status:400},
  {operation:'rename', status:0}, {operation:'open', status:503},
].map(result => ({source, ...result}))))(
  'busy library applies deletion after $operation returns $status through $source', async ({source, operation, status}) => {
  let deleted = false;
  let finishMutation: (() => void) | undefined;
  let finishDeletion: (() => void) | undefined;
  api(c => {
    if (c.method === 'GET') return [200, {videos: deleted ? [] : [video],
      playlists: [{id:'p', name:'Topic', entries: deleted ? [] : ['saved']}]}];
    if (c.body?.action === 'delete_video') {
      deleted = true;
      return new Promise(resolve => { finishDeletion = () => resolve([200, {changed:true}]); });
    }
    return new Promise((resolve, reject) => { finishMutation = () => status === 0
      ? reject(new Error('Unavailable')) : resolve([status, {message:'Mutation failed', changed:status === 200}]); });
  });
  const root = open('/library');
  const other = document.body.appendChild(document.createElement('div'));
  const second = mount(Library, {target:other});
  onTestFinished(() => { finishMutation?.(); finishDeletion?.(); unmount(second); });
  await vi.waitFor(() => expect(text(root)).toContain(video.title));
  await vi.waitFor(() => expect(text(other)).toContain(video.title));
  if (operation === 'rename') {
    button(root, 'Topic (1)').click();
    await vi.waitFor(() => expect($(root, 'section[aria-label="선택한 재생목록"] input')).not.toBeNull());
    type($(root, 'section[aria-label="선택한 재생목록"] input'), 'Renamed');
    button(root, '이름 바꾸기').click();
  } else button($(root, 'li[data-video-key="saved"]')!, '영상 열기').click();
  await vi.waitFor(() => expect(finishMutation).toBeDefined());
  button(other, '영상 삭제').click();
  await vi.waitFor(() => expect($(other, '[role="alertdialog"]')).not.toBeNull());
  button(other, '영구 삭제 확인').click();
  await vi.waitFor(() => expect(finishDeletion).toBeDefined());
  if (source === 'native') {
    for (let i = 0; i < 2; i++) window.dispatchEvent(new CustomEvent('vto-video-deleted', {detail:video.video_key}));
  } else {
    const channel = new BroadcastChannel('vto-library');
    onTestFinished(() => channel.close());
    channel.postMessage(video.video_key); channel.postMessage(video.video_key);
  }
  await new Promise(resolve => setTimeout(resolve, 50));
  expect(calls.filter(c => c.method === 'GET')).toHaveLength(2);
  finishMutation!();
  await vi.waitFor(() => expect($(root, 'li[data-video-key="saved"]')).toBeNull());
  expect(text(root)).toContain('Topic (0)');
  expect(text(root)).not.toContain('저장 자막 있음');
  if (status !== 200) expect($(root, '[role="alert"]')).not.toBeNull();
  expect(calls.filter(c => c.method === 'GET')).toHaveLength(3);
  finishDeletion!();
  await vi.waitFor(() => expect($(other, 'li[data-video-key="saved"]')).toBeNull());
});

test('library loading, failure, retry, and empty guidance', async () => {
  let fail=true;
  api(() => fail ? [503,{message:'Disk unavailable'}] : [200,{videos:[],playlists:[]}]);
  const root=open('/library');
  expect($(root,'[aria-busy="true"]')).not.toBeNull();
  await vi.waitFor(() => expect(text(root)).toContain('Disk unavailable'));
  fail=false; button(root,'다시 불러오기').click();
  await vi.waitFor(() => expect(text(root)).toContain('아직 저장한 영상이 없습니다'));
});

test('saved search, confirmed deletion, cancellation and retry use the named video', async () => {
  let deleted=false; let fail=true;
  api(c => {
    if(c.method==='GET') return [200,{videos:deleted?[]:[video],playlists:[]}];
    if(fail) return [503,{error:'cleanup_failed',message:'Cleanup failed; retry'}];
    deleted=true; return [200,{changed:true}];
  });
  const root=open('/library');
  await vi.waitFor(() => expect(text(root)).toContain(video.title));
  type($(root,'input[type="search"]'),'missing'); await vi.waitFor(() => expect(text(root)).not.toContain(video.title));
  type($(root,'input[type="search"]'),'Attention');
  await vi.waitFor(() => expect(text(root)).toContain(video.title));
  button(root,'영상 삭제').click();
  await vi.waitFor(() => expect($(root,'[role="alertdialog"]')).not.toBeNull());
  expect(text($(root,'[role="alertdialog"]')!)).toContain(video.title);
  expect(text(root)).toContain('API 비용');
  button(root,'취소').click(); expect(calls.filter(c=>c.method==='POST')).toHaveLength(0);
  button(root,'영상 삭제').click();
  await vi.waitFor(() => expect($(root,'[role="alertdialog"]')).not.toBeNull());
  button(root,'영구 삭제 확인').click();
  await vi.waitFor(() => expect(text(root)).toContain('Cleanup failed; retry'));
  expect(calls.find(c => c.method==='POST')?.body).toEqual({action:'delete_video',video_key:'saved',title:video.title,confirmed:true});
  fail=false; button(root,'영구 삭제 확인').click();
  await vi.waitFor(() => expect(text(root)).not.toContain(video.title));
  expect(calls.filter(c => c.method==='POST')).toHaveLength(2);
});

test('playlist controls wire manual order and link addition without creating jobs', async () => {
  api(c => c.method==='GET' ? [200,{videos:[video],playlists:[{id:'p',name:'Topic',entries:['saved']},{id:'q',name:'Other',entries:[]}]}] : [200,{changed:true}]);
  const root=open('/library');
  await vi.waitFor(() => expect(text(root)).toContain('Topic (1)'));
  button(root,'Topic (1)').click();
  await vi.waitFor(() => expect($(root,'input[type="url"]')).not.toBeNull());
  type($(root,'input[type="url"]'),'https://youtu.be/abcDEF12345');
  await vi.waitFor(() => expect(button(root,'링크 추가').disabled).toBe(false));
  button(root,'링크 추가').click();
  await vi.waitFor(() => expect(calls.some(c => c.body?.action==='add_entry')).toBe(true));
  await vi.waitFor(() => expect(calls.filter(c => c.method==='GET')).toHaveLength(2));
  await vi.waitFor(() => expect(button(root,'목록에서 빼기').disabled).toBe(false));
  button(root,'목록에서 빼기').click();
  await vi.waitFor(() => expect(calls.some(c => c.body?.action==='remove_entry')).toBe(true));
  expect(calls.filter(c => c.path==='/jobs')).toHaveLength(0);
});

test('retired library responses do not replace the next screen', async () => {
  let answer: (v: any) => void=()=>{};
  api(c => c.path==='/library/data' ? new Promise(resolve => { answer=resolve; }) : [200,{url:null,job:null}]);
  const root=open('/library');
  button(root,'영상 열기').click();
  await vi.waitFor(() => expect(text(root)).toContain('영상 주소를 넣으면'));
  answer([200,{videos:[{...video,active:true}],playlists:[]}]);
  await new Promise(resolve=>setTimeout(resolve,50));
  expect(text(root)).not.toContain(video.title);
});

test('processing videos keep their source visible and disable deletion', async () => {
  api(() => [200,{videos:[{...video,active:true}],playlists:[]}]);
  const root=open('/library');
  await vi.waitFor(() => expect(text(root)).toContain(video.title));
  expect(button(root,'영상 삭제').disabled).toBe(true);
  expect(text(root)).toContain('처리 중');
  expect(text(root)).toContain(video.page_url);
});
