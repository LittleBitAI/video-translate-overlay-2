import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import {release} from 'node:os';
import {isDeepStrictEqual} from 'node:util';
import {chromium} from 'playwright';
import {digest,sourceSnapshot,assertSameInputs,assertCleanApplication} from './sync-evidence.mjs';

const built = process.argv[2], output = process.argv[3];
assert.match(built ?? '',/^[0-9a-f]{40}$/);assert(output);
assertCleanApplication();
assertSameInputs(sourceSnapshot(built),sourceSnapshot('HEAD'));
const adb = process.env.VTO_ADB;assert(adb,'VTO_ADB must select the installed Android debugging tool');
const phone = args => execFileSync(adb,args,{encoding:'utf8',windowsHide:true}).trim();
const connected = phone(['devices']).split('\n').slice(1).filter(line=>/\sdevice\s*$/.test(line));
assert.equal(connected.length,1,'Exactly one approved phone must be connected');
const serial = connected[0].trim().split(/\s/)[0];
const property = name => phone(['-s',serial,'shell','getprop',name]);
const environment = {windows:release(),android:{model:property('ro.product.model'),release:property('ro.build.version.release'),api:property('ro.build.version.sdk')},
  windows_binary_sha256:digest(readFileSync('target/debug/video-translate-overlay.exe')),
  packaged_apk_sha256:digest(readFileSync('src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk'))};
const browsers = [],devices = [], observations = [];
const wait = async (read, predicate) => {
  const start=Date.now();
  do {const value=await read();if(predicate(value))return value;await new Promise(resolve=>setTimeout(resolve,200));}while(Date.now()-start<20000);
  throw new Error('Native transfer did not converge within 20 seconds');
};
try {
  for (const [name,port] of [['windows',9226],['android',9227]]) {
    const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);browsers.push(browser);
    const page = browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith('http://tauri.localhost'));assert(page);
    assert.equal(await page.evaluate(()=>window.__WIKI_NATIVE_BUILD_HEAD__),built);
    assert.equal(await page.locator('meta[name=wiki-build-head]').getAttribute('content'),built);
    const account = await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('get_google_account'));
    assert.equal(account.state,'signed_in');assert.equal(account.connection?.state,'connected');
    const read = ()=>page.evaluate(g=>window.__TAURI_INTERNALS__.invoke('get_library',{account_generation:g}),account.generation);
    const initial=await read();assert(initial.playlists.length>=2);assert(initial.playlists.every(p=>p.name.startsWith('LAN ')),'Only labeled acceptance fixtures may be renamed');
    assert(initial.videos.every(v=>!v.active));
    const back=page.locator('a[href="/"]');if(await back.count())await back.click();
    if(!(await page.getByRole('heading',{name:'보관함',exact:true}).count()))await page.getByRole('link',{name:'보관함',exact:true}).click();
    devices.push({name,page,read,account,initial});
    observations.push({name:`${name} native identity`,pass:true,actual:{core:built,screen:built,connection:account.connection.state,generation:account.generation,engine:browser.version()}});
  }
  assert(devices[0].account.email===devices[1].account.email,'Matching signed-in accounts are required');
  observations.push({name:'matching signed-in account',pass:true,actual:true});
  for(let i=0;i<2;i++) {
    const source=devices[i],target=devices[1-i],list=source.initial.playlists[i];
    const select = async name => source.page.getByRole('button',{name:`${name} (${list.entries.length})`,exact:true}).click();
    const rename = async name => {await source.page.getByLabel('재생목록 이름',{exact:true}).fill(name);await source.page.getByRole('button',{name:'이름 바꾸기',exact:true}).click();};
    const marker=`LAN review ${source.name} ${Date.now()}`;
    await select(list.name);
    try {
      const start=Date.now();await rename(marker);
      await wait(target.read,data=>data.playlists.some(p=>p.id===list.id&&p.name===marker));
      await target.page.getByRole('button',{name:`${marker} (${list.entries.length})`,exact:true}).waitFor();
      observations.push({name:`${source.name} UI change reaches peer SQLite and UI`,pass:true,actual:{delivery_ms:Date.now()-start,manual_sync:false}});
    } finally {
      await rename(list.name);
      await wait(target.read,data=>data.playlists.some(p=>p.id===list.id&&p.name===list.name));
    }
  }
  for(const device of devices)assert(isDeepStrictEqual(await device.read(),device.initial),'Fixture state must be restored');
  observations.push({name:'both fixture libraries restored after reversible UI changes',pass:true,actual:true});
  const receipt={observed_at:new Date().toISOString(),source_revision:built,environment,observations};
  writeFileSync(output,JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({native_build:built,devices:2,observations:observations.length,passed:true}));
} finally {for(const browser of browsers)await browser.close();}
