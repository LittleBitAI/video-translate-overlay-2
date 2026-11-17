import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';

export async function measure(page) {
  return page.evaluate(() => {
    const box = el => el ? Object.fromEntries(['x','y','width','height','right','bottom'].map(k => [k,el.getBoundingClientRect()[k]])) : null;
    const main = document.querySelector('main');
    const cards = [...document.querySelectorAll('li[data-video-key]')];
    const first = box(cards[0]);
    const account = document.querySelector('section[aria-label="Google 앱 계정"]');
    const key = document.querySelector('#api-key');
    return {viewport:[innerWidth,innerHeight],screenHead:document.querySelector('meta[name="wiki-build-head"]')?.content,
      coreHead:window.__WIKI_NATIVE_BUILD_HEAD__ ?? null,profile:window.__WIKI_NATIVE_PROFILE__ ?? null,
      scrollWidth:document.documentElement.scrollWidth,scrollHeight:document.documentElement.scrollHeight,
      nav:box(document.querySelector('nav')),main:box(main),content:box(main.firstElementChild),stage:box(document.querySelector('.video-stage')),
      firstCard:first,columns:first ? cards.filter(c => Math.abs(c.getBoundingClientRect().top-first.y)<1).length : 0,
      rail:box(document.querySelector('aside')),account:box(account),keyCard:box(key?.closest('section')),key:box(key),
      settingsGroups:[...document.querySelectorAll('.settings-section')].map(row=>({box:box(row),heading:box(row.querySelector('h2')),background:getComputedStyle(row).backgroundColor,radius:getComputedStyle(row).borderRadius})),
      footer:box(document.querySelector('main footer')),keyForm:box(key?.closest('form')),
      workspace:box(key?.closest('section')?.parentElement),
      keyRemove:box(document.querySelector('[data-feature="delete-api-key"]')),
      youtubeLogout:box(document.querySelector('[data-feature="youtube-logout"]')),
      youtubeSection:box(document.querySelector('[data-feature="youtube-logout"]')?.closest('section')),
      update:box([...document.querySelectorAll('main button')].find(el=>el.textContent.trim()==='업데이트 확인')),
      privacyInFooter:[...document.querySelectorAll('main footer button, main footer a')].some(el=>el.textContent.trim()==='개인정보처리방침'),
      navItems:[...document.querySelectorAll('nav a')].map(a => ({text:a.textContent.trim(),href:a.getAttribute('href'),current:a.getAttribute('aria-current')}))};
  });
}

export async function capture(page,out,prefix,binding={}) {
  mkdirSync(out,{recursive:true});
  const results=[];
  for (const [label,name] of [['영상','video'],['보관함','library'],['설정','settings']]) {
    await page.getByRole('link',{name:label,exact:true}).click();
    if(name==='video') await page.locator('.video-stage:not([aria-busy])').waitFor();
    if(name==='library') {
      const row=page.locator('ul[aria-label="재생목록"] li > div > button').first();
      await row.waitFor();
      if(await row.getAttribute('aria-expanded')==='false') await row.click();
      await page.locator('li[data-video-key]').first().waitFor();
    }
    if(name==='settings') await page.locator('#api-key:not(:disabled)').waitFor();
    await page.evaluate(() => document.fonts.ready);
    // Wait for the stage's position-dependent fit after navigation or header changes.
    await page.waitForFunction(() => {
      const stage = document.querySelector('.video-stage');
      return innerWidth < 768 || !stage || stage.getBoundingClientRect().bottom <= innerHeight;
    });
    const r=await measure(page);
    const [width,height]=r.viewport;
    assert(r.scrollWidth<=width,`${name}: horizontal overflow`);
    assert.equal(r.navItems.find(a=>a.current==='page').text,label);
    assert.deepEqual(r.navItems.map(a=>a.href),['/','/library','/preferences']);
    if(width>=768) {
      assert.equal(r.nav.x,16);
      assert.equal(r.main.x,0);
      if(r.stage) {assert.equal(r.stage.x,16);assert(r.stage.bottom<=height);assert(Math.abs(r.stage.width/r.stage.height-16/9)<0.01);}
      if(r.rail) assert.equal(r.rail.x,16);
      if(r.keyCard) {
        assert.equal(r.keyCard.x,16);assert(r.keyForm.width<=960);assert(r.privacyInFooter);
        assert(r.footer.y>=Math.max(...r.settingsGroups.map(group=>group.box.bottom)));
        if(width>=1024) {
          assert.equal(r.youtubeSection.y,r.keyCard.y);assert.equal(r.youtubeSection.bottom,r.keyCard.bottom);
          assert(Math.abs(r.youtubeLogout.bottom-r.keyRemove.bottom)<1);assert(r.account.y>r.keyCard.bottom);
          assert.equal(r.workspace.right,r.main.right-16);assert.equal(r.update.right,r.workspace.right);
          assert(r.update.y<130);assert(r.update.height>=44);
          for(const group of r.settingsGroups) {assert.equal(group.background,'rgba(0, 0, 0, 0)');assert.equal(group.radius,'0px');}
        } else assert(r.account.y>r.keyCard.bottom);
      }
    } else {
      assert.equal(r.nav.bottom,height);
      assert.equal(r.nav.height,45);
      if(r.keyCard) {assert(r.account.y>r.keyCard.bottom);assert(r.privacyInFooter);}
    }
    const filename=`${prefix}-${name}.png`;
    await page.screenshot({path:`${out}/${filename}`});
    results.push({name,...r,...binding,screenshot:filename});
    await page.mouse.move(width-35,Math.min(height-100,400));
    await page.mouse.wheel(0,2000);
    await page.waitForTimeout(200);
    const scrolled=await page.evaluate(()=>({y:scrollY,max:document.documentElement.scrollHeight-innerHeight,width:document.documentElement.scrollWidth}));
    assert(scrolled.width<=width);
    if(scrolled.max>0) assert(scrolled.y>0,`${name}: wheel did not scroll`);
    await page.evaluate(()=>scrollTo(0,0));
    results.at(-1).scroll=scrolled;
    if(name==='library') {
      await page.getByRole('button',{name:'편집',exact:true}).click();
      await page.getByRole('button',{name:'완료',exact:true}).waitFor();
      assert((await measure(page)).scrollWidth<=width);
      await page.getByRole('button',{name:'완료',exact:true}).click();
      await page.getByRole('button',{name:'추가',exact:true}).first().click();
      await page.getByRole('region',{name:'추가',exact:true}).waitFor();
      assert((await measure(page)).scrollWidth<=width);
      await page.getByRole('button',{name:'닫기',exact:true}).click();
    }
  }
  writeFileSync(`${out}/${prefix}.json`,JSON.stringify(results,null,2));
  return results;
}
