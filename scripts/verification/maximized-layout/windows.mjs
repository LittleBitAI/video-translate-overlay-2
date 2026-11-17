import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import {call} from '../library-scenario.mjs';
import {capture,measure} from './capture.mjs';
import {head,environment,inventory,binary,inspect,sha,binding} from './identity.mjs';

const out=process.env.LAYOUT_OUTPUT ?? 'artifacts/maximized-layout/review-1/windows';
const launch=JSON.parse(readFileSync(`${out}/launch.json`));
const treeSha256=inventory(out);
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${launch.port}`,{noDefaults:true});
const requests=[];
const assertions=[];
const check=(name,expected,observed)=>{assertions.push({name,expected,observed});assert.deepEqual(observed,expected,name);};
try {
  const page=browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith('http://tauri.localhost')||p.url().startsWith('tauri://localhost'));
  assert(page,'Native app page was not found');
  await page.getByRole('link',{name:'보관함',exact:true}).waitFor();
  const identity=await page.evaluate(()=>({screen:document.querySelector('meta[name="wiki-build-head"]').content,core:window.__WIKI_NATIVE_BUILD_HEAD__,profile:window.__WIKI_NATIVE_PROFILE__}));
  const buildBinding=binding(identity.core);
  check('native screen and core revisions match',identity.core,identity.screen);
  check('unchanged application inputs at the native revision',[],buildBinding.changedBuildInputs);
  check('disposable native profile',sha(`${launch.profile}\\data`),identity.profile);
  async function request(method,route,body) {
    const result=await call(page,method,route,body);
    requests.push({method,route,body,...result});
    check('native API request succeeded',200,result.status);
    return result.body;
  }
  const initial=await request('GET','/library/data');
  check('fresh native library',{videos:[],playlists:[]},{videos:initial.videos,playlists:initial.playlists});
  await request('POST','/library/data',{action:'create_playlist',name:'Layout fixtures'});
  const lists=(await request('GET','/library/data')).playlists;
  for(let i=0;i<36;i++) await request('POST','/library/data',{action:'add_entry',playlist_id:lists[0].id,
    page_url:`http://127.0.0.1:8146/lecture-${i}.mp4`,title:`Lecture ${i+1}: layout fixture`});
  const sessions=[];
  for(const state of ['Normal','Maximized','Normal']) {
    const native=JSON.parse(execFileSync('pwsh',['-NoProfile','-File','scripts/verification/maximized-layout/native-state.ps1',
      '-AppPid',String(launch.pid),'-State',state],{encoding:'utf8',windowsHide:true}));
    const ownership=inspect(launch.pid,launch.port);
    const prefix=`${sessions.length}-${state.toLowerCase()}`;
    const results=await capture(page,out,prefix);
    await page.getByRole('link',{name:'영상',exact:true}).click();
    await page.locator('.video-stage:not([aria-busy])').waitFor();
    await page.evaluate(async()=>{
      const account=await window.__TAURI_INTERNALS__.invoke('get_google_account',{});
      window.vtoReady({text:'Saved subtitles are ready. '.repeat(8),url:'http://127.0.0.1:8146/layout.mp4',account_generation:account.generation});
    });
    await page.locator('#address').fill('file:///outside/target.mp4');
    await page.getByRole('button',{name:'열기',exact:true}).click();
    await page.locator('#address-error').waitFor();
    await page.waitForFunction(()=>document.querySelector('.video-stage').getBoundingClientRect().bottom<=innerHeight);
    const messages=await measure(page);
    check('native messages leave the stage visible',true,messages.stage.bottom<=messages.viewport[1]);
    await page.screenshot({path:`${out}/${prefix}-messages.png`});
    sessions.push({native,ownership,results,messages});
    console.log(`${state}: ${results[0].viewport.join('x')}; ${results[1].columns} library columns; no overflow`);
    // Navigation retires the ready notice and address error before the next window transition.
    await page.getByRole('link',{name:'보관함',exact:true}).click();
  }
  environment.webview=browser.version();
  writeFileSync(`${out}/receipt.json`,JSON.stringify({head,buildBinding,treeSha256,environment,binary:binary('target/debug/video-translate-overlay.exe'),
    identity,launch:{pid:launch.pid,port:launch.port,profile:identity.profile,hostSources:launch.hostSources},requests,assertions,sessions},null,2)+'\n');
  console.log(`PASS: ${assertions.length} native assertions; core and screen ${identity.core}`);
} finally {await browser.close();}
