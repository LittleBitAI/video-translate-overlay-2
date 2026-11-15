import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,readdir,mkdir,writeFile} from 'node:fs/promises';
import {chromium} from 'playwright';

const directory=new URL('../docs/branding/',import.meta.url);
const assets=['.nojekyll','index.html','privacy.html','style.css'];
assert.deepEqual((await readdir(directory)).sort(),assets);
const bodies=new Map(await Promise.all(assets.map(async name=>[name,await readFile(new URL(name,directory))])));
for(const [name,body] of bodies){
  assert(!body.subarray(0,3).equals(Buffer.from([0xef,0xbb,0xbf])),`${name}: BOM`);
  const text=body.toString('utf8');
  assert(!/PUBLIC_SUPPORT_EMAIL|HOME_PAGE_URL|HOSTING_PRIVACY_NOTICE|__SUPPORT_|GOCSPX-|AIza/.test(text),`${name}: unpublished placeholder or credential`);
  if(name.endsWith('.html')){
    assert(!/<(?:script|form|iframe)\b/i.test(text),`${name}: active data collection surface`);
    assert.equal((text.match(/<h1\b/g)||[]).length,1);
  }
}
const prefix='/video-translate-overlay-2/';
const server=createServer((req,res)=>{
  const route=req.url===prefix?'index.html':req.url?.startsWith(prefix)?req.url.slice(prefix.length):'';
  const body=bodies.get(route);
  if(req.method!=='GET'||!body){res.writeHead(404);res.end();return;}
  res.writeHead(200,{'Content-Type':route.endsWith('.css')?'text/css; charset=utf-8':'text/html; charset=utf-8'});res.end(body);
});
await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
let browser;
try{
  browser=await chromium.launch({headless:true});
  const page=await browser.newPage();
  const origin=`http://127.0.0.1:${server.address().port}`;
  const results=[];
  for(const width of [384,400,1280]){
    await page.setViewportSize({width,height:854});
    await page.goto(origin+prefix);
    const nav=page.getByRole('navigation');
    await nav.getByRole('link',{name:'Privacy policy',exact:true}).click();
    assert.equal(new URL(page.url()).pathname,prefix+'privacy.html');
    await page.getByRole('heading',{level:1,name:'Video Translate Overlay privacy policy'}).waitFor();
    assert((await page.textContent('main')).includes('GitHub logs and stores visitors'));
    const privacyContact=await nav.getByRole('link',{name:'Support',exact:true}).getAttribute('href');
    await nav.getByRole('link',{name:'Home',exact:true}).click();
    assert.equal(new URL(page.url()).pathname,prefix);
    assert.equal(await nav.getByRole('link',{name:'Support',exact:true}).getAttribute('href'),privacyContact);
    for(const path of ['', 'privacy.html']){
      await page.goto(origin+prefix+path);
      const metrics=await page.evaluate(()=>({
        width:innerWidth,scroll:document.documentElement.scrollWidth,
        navTargets:[...document.querySelectorAll('nav a')].map(a=>a.getBoundingClientRect().height),
        bodySize:getComputedStyle(document.body).fontSize,
        headingSize:getComputedStyle(document.querySelector('h1')).fontSize,
        sectionSize:getComputedStyle(document.querySelector('h2')).fontSize,
        externalScripts:document.scripts.length,
        links:[...document.querySelectorAll('a')].map(a=>a.href)
      }));
      assert(metrics.scroll<=metrics.width,`${path}: horizontal overflow at ${width}`);
      assert(metrics.navTargets.every(height=>height>=44));
      assert.equal(metrics.bodySize,'16px');assert.equal(metrics.headingSize,'28px');assert.equal(metrics.sectionSize,'20px');
      assert.equal(metrics.externalScripts,0);
      assert(metrics.links.every(link=>link.startsWith(origin+prefix)||link.startsWith('https://')||/^mailto:[^\s@]+@[^\s@]+$/.test(link)));
      results.push({width,page:path||'index.html',horizontalOverflow:false,minNavigationTarget:Math.min(...metrics.navTargets)});
    }
  }
  const privatePath=await fetch(origin+prefix+'../../artifacts/google-oauth/desktop-client.json');assert.equal(privatePath.status,404);
  const markup=new TextDecoder().decode(bodies.get('index.html'));
  assert(markup.includes('0.1.7')&&markup.includes('development features')&&markup.includes('./privacy.html'));
  await mkdir(new URL('../artifacts/branding/',import.meta.url),{recursive:true});
  await writeFile(new URL('../artifacts/branding/layout-check.json',import.meta.url),JSON.stringify(results,null,2)+'\n');
  await page.setViewportSize({width:384,height:854});await page.goto(origin+prefix);
  await page.screenshot({path:new URL('../artifacts/branding/homepage-phone.png',import.meta.url).pathname.replace(/^\/([A-Za-z]:)/,'$1'),fullPage:true});
  console.log(`Branding checks passed: ${results.length} layouts, both navigation directions, consistent public support contact, no active collection or private-file route`);
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
