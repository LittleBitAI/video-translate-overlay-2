// Actual local HTTP or native IPC reads around a repair in a disposable copy.
import assert from 'node:assert/strict';
import {spawn, spawnSync, execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdirSync,copyFileSync,readFileSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import os from 'node:os';
import {chromium} from 'playwright';
import {call} from './library-scenario.mjs';

const [mode, executable, repairTool, fixture, audio, models, destination] = process.argv.slice(2);
assert(['api','native'].includes(mode) && destination);
const out=resolve(destination), profile=resolve(out,'profile'), key='QDX-1M5Nj7s';
mkdirSync(out,{recursive:true});
mkdirSync(profile); // Refuse to reuse a possibly live or already repaired profile.
copyFileSync(fixture,resolve(profile,'app.sqlite3'));
writeFileSync(resolve(profile,'update-fixture.json'),'[]'); // Keep startup update checks local in this debug-only profile.
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const digest=value=>sha(JSON.stringify(value));
const head=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
const requests=[], assertions=[], identities=[];
const check=(name,expected,observed)=>{assert.deepEqual(observed,expected,name);assertions.push({name,expected,observed});};
const cues=value=>value.cues.map(({seq,...cue})=>cue);
const video=library=>library.videos.find(v=>v.video_key===key);
const selection=library=>video(library).versions.find(v=>v.selected).id;
let child, browser, page, log;
async function stop() {
  if(browser){await browser.close();browser=undefined;}
  if(child && child.exitCode===null) {
    const exit=new Promise(r=>child.once('exit',r));
    if(mode==='native') spawnSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});
    else child.kill();
    await exit;
  }
  child=undefined;
  if(log!==undefined){closeSync(log);log=undefined;}
}
async function start() {
  log=openSync(resolve(out,'host.log'),'a');
  child=spawn(resolve(executable),mode==='api'?['--stand-ins']:[],{windowsHide:true,stdio:['ignore',log,log],env:{...process.env,
    PORT:'23891',APP_DATA_DIR:profile,WIKI_NATIVE_PROFILE:profile,
    WEBVIEW2_USER_DATA_FOLDER:resolve(out,'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-address=127.0.0.1 --remote-debugging-port=9239'}});
  for(let attempt=0;attempt<200;attempt++) {
    assert.equal(child.exitCode,null,'owned host exited');
    try {
      if(mode==='api') {
        const response=await fetch('http://127.0.0.1:23891/build');
        const identity=await response.json();
        check('HTTP host process ownership',child.pid,identity.pid);
        check('HTTP host source revision',head,identity.head);
        identities.push(identity);
      } else {
        browser=await chromium.connectOverCDP('http://127.0.0.1:9239',{noDefaults:true});
        page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith('http://tauri.localhost')||p.url().startsWith('tauri://localhost'));
        assert(page);
        const identity=await page.evaluate(()=>({core:window.__WIKI_NATIVE_BUILD_HEAD__,screen:document.querySelector('meta[name=wiki-build-head]')?.content,profile:window.__WIKI_NATIVE_PROFILE__}));
        assert(identity.core && identity.screen);
        check('isolated native profile',sha(profile),identity.profile);
        check('native core source revision',head,identity.core);
        check('native core and screen revision',identity.core,identity.screen);
        identities.push({...identity,engine:browser.version(),pid:child.pid});
      }
      return;
    } catch {
      if(browser){await browser.close();browser=undefined;}
      await new Promise(r=>setTimeout(r,100));
    }
  }
  throw new Error('owned host startup timed out');
}
async function request(method,route,body={}) {
  let result;
  if(mode==='native') result=await call(page,method,route,body);
  else {
    const response=await fetch('http://127.0.0.1:23891'+route,{method,...(method==='POST'?{headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}:{})});
    result={status:response.status,body:await response.json()};
  }
  check('successful local request',200,result.status);
  // This is the single publication boundary: expose only the named public lecture and bounded digests.
  let projection;
  if(route==='/library/data' && method==='POST') projection={changed:result.body.changed};
  else if(route==='/library/data') projection={target:video(result.body),playlist_count:result.body.playlists.length,playlists_sha256:digest(result.body.playlists)};
  else if(route.includes('/subtitles')) projection={cue_count:result.body.cues.length,cues_sha256:digest(cues(result.body)),first_cues:result.body.cues.slice(0,2).map(c=>({seq:c.seq,start_ms:c.start_ms,end_ms:c.end_ms,source_sha256:digest(c.source_text),translation_sha256:digest(c.text_ko)}))};
  else projection={cost_krw:result.body.cost_krw,stage:result.body.stage};
  if(projection.target) projection.target={video_key:key,saved:projection.target.saved,versions:projection.target.versions};
  requests.push({method,route,body,status:result.status,response:projection});
  return result.body;
}
try {
  await start();
  const before=await request('GET','/library/data');
  const old=await request('GET',`/videos/${key}/subtitles`);
  const previous=selection(before);
  check('original cue count',755,old.cues.length);
  check('reported music interval',[1180,5800],[old.cues[0].start_ms,old.cues[0].end_ms]);
  const others=before.videos.filter(v=>v.saved&&v.video_key!==key);
  const unrelated=[];
  // Other libraries are compared privately; only an aggregate digest leaves this producer.
  for(const v of others) {
    const route=`/videos/${encodeURIComponent(v.video_key)}/subtitles`;
    const result=mode==='native'?await call(page,'GET',route):{body:await(await fetch('http://127.0.0.1:23891'+route)).json()};
    unrelated.push(result.body);
  }
  const repair=spawnSync(resolve(repairTool),[resolve(profile,'app.sqlite3'),key,previous,'0',resolve(audio),resolve(models),'--apply'],{encoding:'utf8',windowsHide:true});
  check('repair process exit',0,repair.status);
  assert.match(repair.stdout,/Published t\/QDX-1M5Nj7s\//);
  const after=await request('GET','/library/data');
  const fresh=await request('GET',`/videos/${key}/subtitles`);
  check('remaining speech and timing bytes',digest(cues(old).slice(1)),digest(cues(fresh)));
  check('repaired cue count',754,fresh.cues.length);
  check('first speech source time',9100,fresh.cues[0].start_ms);
  check('retained saved versions',2,video(after).versions.length);
  check('previous version remains available',true,video(after).versions.some(v=>v.id===previous));
  check('playlists unchanged',digest(before.playlists),digest(after.playlists));
  const selected=selection(after);
  assert.notEqual(selected,previous);
  const afterOthers=[];
  for(const v of others) {
    const route=`/videos/${encodeURIComponent(v.video_key)}/subtitles`;
    const result=mode==='native'?await call(page,'GET',route):{body:await(await fetch('http://127.0.0.1:23891'+route)).json()};
    afterOthers.push(result.body);
  }
  check('unrelated saved responses unchanged',digest(unrelated),digest(afterOthers));
  check('unrelated saved response count',4,others.length);
  const jobRoute='/jobs'+(mode==='api'?'?page_url='+encodeURIComponent('https://www.youtube.com/watch?v='+key):'');
  const job=await request('GET',jobRoute,{page_url:'https://www.youtube.com/watch?v='+key});
  check('cumulative cost retained',276.2938750000004,job.cost_krw);
  for(const id of [previous,selected]) {
    await request('POST','/library/data',{action:'select_translation',video_key:key,translation_id:id});
    check('explicit saved-version selection',id,selection(await request('GET','/library/data')));
    check('selected response cue count',id===previous?755:754,(await request('GET',`/videos/${key}/subtitles`)).cues.length);
  }
  await stop();await start();
  check('restart preserves repaired subtitles',digest(fresh),digest(await request('GET',`/videos/${key}/subtitles`)));
  check('restart preserves repaired selection',selected,selection(await request('GET','/library/data')));
  const inputs=execFileSync('git',['ls-files','Cargo.toml','Cargo.lock','package.json','package-lock.json','index.html','vite*.ts','src','src-tauri'],{encoding:'utf8'}).trim().split('\n').filter(p=>!p.endsWith('.md'));
  const result={collector_head:head,mode,environment:{observed_at:new Date().toISOString(),os:os.version(),release:os.release(),arch:os.arch(),node:process.version},
    build_inputs:inputs.map(path=>({path,sha256:sha(readFileSync(path))})),
    binaries:{host:{name:basename(executable),sha256:sha(readFileSync(executable))},repair:{name:basename(repairTool),sha256:sha(readFileSync(repairTool))}},identities,requests,assertions,
    limits:{disposable_copy:true,production_modified:false,translation_requests:0,native_ui_actions:false,playback_observation:'earlier owner report; no new playback claim'}};
  writeFileSync(resolve(out,'observations.json'),JSON.stringify(result,null,2)+'\n');
  console.log(`PASS: ${mode}; ${requests.length} local requests; ${assertions.length} assertions; repair, selection and real process restart`);
} finally {await stop();}
