import assert from 'node:assert/strict';
import {chromium} from 'playwright';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {call} from '../../../../scripts/verification/library-scenario.mjs';
const out='artifacts/subtitle-scaling';
const head=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const receipts={head,at:new Date().toISOString(),native:[],browser:[]};
async function measure(page,selector) {
 return page.locator(selector).evaluate(sub=>{
  const v=document.querySelector('video'),b=v.getBoundingClientRect(),scale=Math.min(b.width/v.videoWidth,b.height/v.videoHeight);
  const w=v.videoWidth*scale,h=v.videoHeight*scale;
  return {viewport:[innerWidth,innerHeight],image:{left:b.left+(b.width-w)/2,top:b.top+(b.height-h)/2,width:w,height:h},
   subtitle:sub.getBoundingClientRect().toJSON(),text:sub.textContent.trim(),font:parseFloat(getComputedStyle(sub).fontSize),
   scroll:[sub.scrollWidth,sub.clientWidth],time:v.currentTime,paused:v.paused,duration:v.duration,fullscreen:!!document.fullscreenElement,
   overflow:document.documentElement.scrollWidth>innerWidth};
 });
}
async function capture(page,kind,name,selector,text) {
 await page.waitForFunction(({selector,text})=>{
  const sub=selector.includes('data-vto')?document.querySelector('[data-vto="overlay"]')?.shadowRoot.querySelector('.sub'):document.querySelector(selector);
  return sub?.textContent.trim()===text;
 },{selector,text});
 await page.waitForTimeout(300);
 const r=await measure(page,selector),image=r.image,s=r.subtitle;
 assert(s.bottom<=image.top+image.height-Math.max(48,image.height*.1),'Player controls stay clear');
 assert.equal(r.text,text); assert(s.left>=image.left-1 && s.right<=image.left+image.width+1,'horizontal image fit');
 assert(s.top>=image.top && s.bottom<=image.top+image.height,'vertical image fit');
 assert(r.scroll[0]<=r.scroll[1]+1,'text wraps'); assert.equal(r.overflow,false);
 assert(Math.abs(r.font-Math.max(14,Math.min(image.width*.03,image.height*.055)))<.1,'size follows image');
 await page.screenshot({path:`${out}/${kind}-${name}.png`});
 receipts[kind].push({name,...r});console.log(`${kind}/${name}: ${r.viewport.join('x')}; ${r.font}px; cue at ${r.time.toFixed(2)}s; fits`);
 return r;
}
mkdirSync(out,{recursive:true});
const launch=JSON.parse(readFileSync(`${out}/windows-final/launch.json`));
const native=await chromium.connectOverCDP(`http://127.0.0.1:${launch.port}`,{noDefaults:true});
try {
 const pages=()=>native.contexts().flatMap(c=>c.pages());
 const main=pages().find(p=>p.url().includes('tauri.localhost'));
 receipts.identity=await main.evaluate(()=>({head:window.__WIKI_NATIVE_BUILD_HEAD__,profile:window.__WIKI_NATIVE_PROFILE__,screen:document.querySelector('meta[name="wiki-build-head"]').content}));
 assert.equal(receipts.identity.screen,receipts.identity.head);
 assert.equal(execFileSync('git',['diff','--name-only',receipts.identity.head,head,'--','src','src-tauri','package.json','package-lock.json','vite.config.ts','vite.overlay.config.ts'],{encoding:'utf8'}).trim(),'','Native application inputs match');
 assert.equal(receipts.identity.profile,sha(`${launch.profile}\\data`),'Disposable native profile');
 receipts.exe={sha256:sha(readFileSync('target/debug/video-translate-overlay.exe'))};
 receipts.ownership=JSON.parse(execFileSync('pwsh',['-NoProfile','-File','scripts/verification/maximized-layout/inspect.ps1','-AppPid',String(launch.pid),'-Port',String(launch.port),'-RootPid',String(launch.pid)],{encoding:'utf8',windowsHide:true}));
 receipts.webview=native.version();
 assert.equal((await call(main,'POST','/browser/open',{url:'http://127.0.0.1:8159/saved.mp4'})).status,200);
 await main.waitForTimeout(1200);
 const site=pages().find(p=>p.url().includes('8159/saved.mp4'));
 assert(site,'Native player opened');
 if(await site.evaluate(()=>!!document.fullscreenElement)) await site.locator('[data-vto="fullscreen"]').click();
 receipts.restoredWindow=JSON.parse(execFileSync('pwsh',['-NoProfile','-File','docs/evidence/fullscreen-subtitles/scripts/native-window.ps1','-AppPid',String(launch.pid)],{encoding:'utf8',windowsHide:true}));
 const api=await call(main,'GET','/library/data');assert.equal(api.status,200);
 const saved=api.body.videos.find(v=>v.page_url.includes('/saved.mp4'));
 const before=await call(main,'GET',`/videos/${saved.video_key}/subtitles`);assert.equal(before.status,200);
 const cues=before.body.cues;
 const seek=async seconds=>{await site.locator('video').evaluate((v,t)=>{v.pause();v.currentTime=t;},seconds);};
 await seek(.5);await capture(site,'native','restored-short','[data-vto="subtitle"]',cues[0].text_ko);
 await site.locator('[data-vto="fullscreen"]').click();await site.waitForFunction(()=>innerWidth===1920&&innerHeight===1080);
 const full=await capture(site,'native','fullscreen-short','[data-vto="subtitle"]',cues[0].text_ko);assert(full.font>26);
 await seek(1.5);await capture(site,'native','fullscreen-long','[data-vto="subtitle"]',cues[1].text_ko);
 await site.locator('video').evaluate(v=>v.play());await site.waitForTimeout(250);await site.locator('video').evaluate(v=>v.pause());
 const frozen=await site.locator('video').evaluate(v=>v.currentTime);await site.waitForTimeout(250);
 assert(frozen>1.5,'Actual playback advances the source clock');
 assert.equal(await site.locator('video').evaluate(v=>v.currentTime),frozen,'pause freezes source clock');
 await seek(.6);await capture(site,'native','seek-back','[data-vto="subtitle"]',cues[0].text_ko);
 await seek(1.6);await site.locator('video').evaluate(v=>v.style.height='80%');
 await capture(site,'native','letterboxed-long','[data-vto="subtitle"]',cues[1].text_ko);
 await site.locator('video').evaluate(v=>v.style.height='100%');
 await site.locator('[data-vto="fullscreen"]').click();await site.waitForFunction(()=>!document.fullscreenElement);
 await capture(site,'native','exit-long','[data-vto="subtitle"]',cues[1].text_ko);
 receipts.resizedWindow=JSON.parse(execFileSync('pwsh',['-NoProfile','-File','docs/evidence/fullscreen-subtitles/scripts/native-window.ps1','-AppPid',String(launch.pid),'-Width','976','-Height','639'],{encoding:'utf8',windowsHide:true}));
 await capture(site,'native','resized-long','[data-vto="subtitle"]',cues[1].text_ko);
 assert.deepEqual(await call(main,'GET',`/videos/${saved.video_key}/subtitles`),before,'cues and timestamps unchanged');
 receipts.nativeCueDigest=sha(JSON.stringify(before.body));
} finally {await native.close();}
const browser=await chromium.launch({headless:true});
try {
 const page=await browser.newPage();
 const opened=await fetch('http://127.0.0.1:8157/browser/open',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({url:'http://127.0.0.1:8159/other.mp4'})});assert.equal(opened.status,200);
 receipts.browserCore=await (await fetch('http://127.0.0.1:8157/build')).json();
 assert.equal(execFileSync('git',['diff','--name-only',receipts.browserCore.head,head,'--','Cargo.toml','Cargo.lock','src-tauri/src','src-tauri/build.rs'],{encoding:'utf8'}).trim(),'','Reused browser core is unchanged');
 const info=await (await fetch('http://127.0.0.1:8157/browser')).json();
 const body=await (await fetch(`http://127.0.0.1:8157/videos/${info.job.video_key}/subtitles`)).json();
 for(const [w,h] of [[1280,720],[1920,1080],[384,854]]) {
  await page.setViewportSize({width:w,height:h});await page.goto('http://127.0.0.1:8157/');
  assert.equal(await page.locator('meta[name=\"wiki-build-head\"]').getAttribute('content'),receipts.identity.screen,'Browser screen matches native screen');
  await page.waitForFunction(()=>document.querySelector('video')?.readyState>=2);
  await page.locator('video').evaluate(v=>v.play());await page.waitForTimeout(200);
  await page.locator('video').evaluate(v=>{v.pause();v.currentTime=.5;});
  await page.waitForFunction(()=>!document.querySelector('video').seeking);
  await capture(page,'browser',`${w}x${h}`,'[data-feature="watch-subtitles"]',body.cues[0].text_ko);
 }
} finally {await browser.close();}
writeFileSync(`${out}/receipt.json`,JSON.stringify(receipts,null,2)+'\n');
console.log(`PASS: ${receipts.native.length} actual Windows states and ${receipts.browser.length} browser viewports; saved cues unchanged`);
