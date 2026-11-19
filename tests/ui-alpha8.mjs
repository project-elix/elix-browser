// Real Chromium shell interactions against a strict in-memory Tauri RPC fixture.
// Native WebView2, SQLite and Windows window behavior are covered separately in CI.
import {createServer} from 'vite';
import {fileURLToPath} from 'node:url';
import {resolve,join} from 'node:path';
import {mkdir} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {chromium} from 'playwright';
const root=fileURLToPath(new URL('..',import.meta.url));
const output=resolve(process.env.ELIX_UI_OUTPUT||join(root,'..','alpha8-ui'));
await mkdir(output,{recursive:true});
const server=await createServer({root,server:{host:'127.0.0.1',port:4188,strictPort:true}});
await server.listen();
let browser;
try{browser=await chromium.launch({...(process.env.CHROMIUM_PATH?{executablePath:process.env.CHROMIUM_PATH}:{}),headless:true})}catch(error){await server.close();throw error}
const page=await browser.newPage({viewport:{width:1440,height:950}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
page.setDefaultTimeout(6000);
// Cold Vite dependency transformation on Windows is setup, not an interaction latency target.
page.setDefaultNavigationTimeout(45000);
try {
 await page.addInitScript(()=>{
  const tab=(id,url,title)=>({id,url,title,private:false,pinned:false,workspace:'Personlig',group:'',muted:false,loading:false,suspended:false,last_active:0,load_ms:null,http_status:null,navigation_error:null,favicon:null});
  const state={session:{tabs:[tab('original','https://example.com/form','Original page'),tab('peer','https://example.com/video','Other page'),tab('third','https://example.com/third','Third page')],active:'original',workspace:'Personlig',workspaces:['Personlig','Jobb'],closed:[],split:null},profiles:[{id:'test-profile',name:'Andreas'},{id:'other-profile',name:'Andre'}],profile:'test-profile',settings:{appearance:{accent:'#43d3e8',background:'#080b12',density:'comfortable',radius:12,reduce_motion:false,gradient:true,sidebar:'expanded',bookmark_bar:'toolbar'},developer:false,gaming:false,channel:'developer',suspend_minutes:0,password_autosave:false,autofill:false},version:'0.1.2-dev.8',updater_configured:true};
  const entry=(id,title,group='')=>({id,title,url:'https://example.com/'+id,group,favicon:null,time:'2026-11-17T12:00:00Z'});
  const libraries={'test-profile':{items:[entry(1,'ELIX bookmark'),entry(2,'Work bookmark','Arbeid')],groups:['Arbeid','Tom gruppe']},'other-profile':{items:[entry(9,'Other profile bookmark')],groups:['Andre ting']}};
  let serial=0;const callbacks=new Map(),listeners=new Map(),calls=[];
  const emit=(event,payload)=>{for(const [id,v]of listeners)if(v.event===event)callbacks.get(v.handler)?.({event,id,payload:structuredClone(payload)})};
  window.__uiTest={state,libraries,calls,emit};window.isTauri=true;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:(_,id)=>listeners.delete(id)};
  window.__TAURI_INTERNALS__={transformCallback:cb=>{const id=++serial;callbacks.set(id,cb);return id},invoke:async(cmd,args)=>{
   if(cmd==='plugin:event|listen'){const id=++serial;listeners.set(id,args);return id}
   if(cmd==='plugin:event|unlisten')return;
   if(cmd!=='dispatch')throw Error('Unexpected command '+cmd);
   const r=args.request;calls.push(structuredClone(r));
   const lib=libraries[state.profile];
   if(['bookmark_groups','create_bookmark_group','rename_bookmark_group','remove_bookmark_group','move_bookmark','edit_bookmark'].includes(r.action)&&r.profile!==state.profile)throw Error('Stale or missing profile');
   switch(r.action){
    case 'snapshot':return structuredClone(state);
    case 'list_entries':return r.kind==='bookmarks'?structuredClone(lib.items):[];
    case 'bookmark_groups':return structuredClone(lib.groups);
    case 'downloads':case 'site_permissions':return [];
    case 'window':return {maximized:false,fullscreen:false};
    case 'check_update':return {state:'current'};
    case 'account':return {configured:true,account:null,email:true,sync:true,discord:false};
    case 'media':return {supported:true,player:null,fullscreen:null};
    case 'extensions':return {supported:true,extensions:[]};
    case 'window_ready':case 'bounds':return;
    case 'activate':state.session.active=r.id;break;
    case 'navigate':{const t=state.session.tabs.find(t=>t.id===r.id);t.url=r.url;break;}
    case 'settings':state.settings=r.settings;return;
    case 'switch_profile':state.profile=r.id;return;
    case 'new_tab':{const id='new-'+(++serial),t=tab(id,r.url,'ELIX');t.private=!!r.private;state.session.tabs.push(t);state.session.active=id;emit('browser-changed',state.session);return id;}
    case 'split':state.session.split=r.other?[state.session.active,r.other]:null;break;
    case 'close_tab':state.session.tabs=state.session.tabs.filter(t=>t.id!==r.id);state.session.active=state.session.tabs.at(-1).id;break;
    case 'group_tab':{const t=state.session.tabs.find(t=>t.id===r.id);t.group=r.group;t.workspace=r.workspace;break;}
    case 'workspace':state.session.workspace=r.name;if(!state.session.workspaces.includes(r.name))state.session.workspaces.push(r.name);break;
    case 'reorder_tab':{const from=state.session.tabs.findIndex(t=>t.id===r.id),[t]=state.session.tabs.splice(from,1),to=r.before?state.session.tabs.findIndex(t=>t.id===r.before):state.session.tabs.length;if(r.ungroup_at)t.group='';state.session.tabs.splice(to,0,t);break;}
    case 'create_bookmark_group':lib.groups.push(r.name);emit('bookmarks-changed',null);return;
    case 'rename_bookmark_group':lib.groups=lib.groups.map(g=>g===r.name?r.new_name:g);for(const i of lib.items)if(i.group===r.name)i.group=r.new_name;emit('bookmarks-changed',null);return;
    case 'remove_bookmark_group':lib.groups=lib.groups.filter(g=>g!==r.name);for(const i of lib.items)if(i.group===r.name)i.group='';emit('bookmarks-changed',null);return;
    case 'move_bookmark':lib.items.find(i=>i.id===r.id).group=r.group;emit('bookmarks-changed',null);return;
    case 'edit_bookmark':Object.assign(lib.items.find(i=>i.id===r.id),{title:r.title,url:r.url});emit('bookmarks-changed',null);return;
    case 'remove_bookmark':if(r.profile!==state.profile)throw Error('Stale profile');lib.items=lib.items.filter(i=>i.id!==r.id);emit('bookmarks-changed',null);return;
    default:throw Error('Unexpected action '+r.action);
   }
   emit('browser-changed',state.session);
  }};
 });
 await page.goto('http://127.0.0.1:4188');
 await page.getByRole('tab',{name:'Original page',exact:true}).waitFor();

 // WebView2 can report a redirect in the active tab after the user has started editing its address.
 {
  const field=page.getByRole('combobox',{name:'Adressefelt',exact:true});
  const originalUrl='https://example.com/form';
  const publishUrl=async url=>{
   await page.evaluate(value=>{
    const {state,emit}=window.__uiTest;
    const active=state.session.tabs.find(tab=>tab.id===state.session.active);
    active.url=value;
    active.loading=false;
    emit('browser-changed',state.session);
   },url);
   // Read the controlled input after React has committed the event and its effects.
   await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  };
  const redirected='https://example.com/finished-loading';
  await field.click();
  await field.fill('søk: mitt utkast');
  await publishUrl(redirected);
  assert.equal(await field.inputValue(),'søk: mitt utkast','a same-tab navigation overwrote the typed draft');
  assert.equal(await field.getAttribute('aria-expanded'),'true','the redirect closed active suggestions');

  await field.press('Escape');
  assert.equal(await field.getAttribute('aria-expanded'),'false');
  assert.equal(await field.inputValue(),redirected,'Escape should restore the latest actual URL');
  await field.press('Tab');
  const passive='https://example.com/passive';
  await publishUrl(passive);
  assert.equal(await field.inputValue(),passive,'an unfocused address must track the active page');
  await field.click();
  await field.press('Control+a');
  const range=()=>field.evaluate(el=>[el.selectionStart,el.selectionEnd]);
  assert.deepEqual(await range(),[0,passive.length]);
  await publishUrl('https://example.com/updated-before-paste');
  assert.equal(await field.inputValue(),passive,'a redirect changed the address selected for replacement');
  assert.deepEqual(await range(),[0,passive.length],'a redirect lost the pending replacement selection');
  await page.context().grantPermissions(['clipboard-read','clipboard-write'],{origin:'http://127.0.0.1:4188'});
  await page.evaluate(()=>navigator.clipboard.writeText('søk: lim inn'));
  await field.press('Control+v');
  assert.equal(await field.inputValue(),'søk: lim inn','paste should replace the entire selected address');
  await field.press('Tab');
  assert.equal(await field.inputValue(),'https://example.com/updated-before-paste','blur left a stale draft');

  await field.click();
  await field.fill('uferdig');
  await page.getByRole('tab',{name:'Other page',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('[aria-label="Adressefelt"]')?.value==='https://example.com/video');
  await page.getByRole('tab',{name:'Original page',exact:true}).click();
  await field.click();
  await field.fill('ferdig: send hele søket');
  await field.press('Enter');
  await page.waitForFunction(()=>window.__uiTest.calls.some(c=>
   c.action==='navigate'&&c.id==='original'&&c.url==='ferdig: send hele søket'));
  await publishUrl(originalUrl);
  assert.equal(await field.inputValue(),originalUrl,'the submitted edit still blocked subsequent redirects');
  await field.press('Tab');
  console.log('PASS: redirects preserve drafts and select-all/paste, with Escape, blur, switching and submit');
 }

 const gear=page.getByRole('button',{name:'Innstillinger og hurtigvalg',exact:true});
 const quick=page.getByRole('region',{name:'Hurtigvalg',exact:true});
 assert.equal(await quick.count(),0,'quick settings are hidden initially');
 assert.equal(await page.getByRole('button',{name:'Delt visning',exact:true}).count(),0);
 assert.equal(await page.getByRole('combobox',{name:'Arbeidsområde',exact:true}).count(),0);
 const beforeTabs=await page.evaluate(()=>window.__uiTest.state.session.tabs.length);
 await gear.click();
 await quick.waitFor();
 // A queued notification from the old page can arrive after the user clicked the gear.
 await page.evaluate(()=>window.__uiTest.emit('page-focused','original'));
 await quick.getByRole('button',{name:'Delt visning',exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.length),beforeTabs,'single gear click does not navigate');
 for(const name of ['Delt visning','Faner','Arbeidsområde'])assert.equal(await quick.getByRole('button',{name,exact:true}).getAttribute('aria-expanded'),'false');
 await quick.getByRole('button',{name:'Delt visning',exact:true}).click();
 await quick.getByRole('button',{name:'Åpne delt visning',exact:true}).click();
 await page.waitForFunction(()=>window.__uiTest.state.session.split?.length===2);
 await quick.getByRole('button',{name:'Avslutt delt visning',exact:true}).click();
 await page.waitForFunction(()=>!window.__uiTest.state.session.split);
 await quick.getByRole('button',{name:'Arbeidsområde',exact:true}).click();
 await quick.getByRole('combobox',{name:'Arbeidsområde',exact:true}).waitFor();
 assert.equal(await quick.getByRole('button',{name:'Åpne delt visning',exact:true}).count(),0,'accordion closes previous choice');
 await page.keyboard.press('Escape');
 assert.equal(await quick.count(),0);
 assert.equal(await gear.evaluate(element=>element===document.activeElement),true,'Escape restores gear focus');
 await gear.click();await quick.waitFor();
 await page.getByRole('tab',{name:'Original page',exact:true}).click();
 assert.equal(await quick.count(),0,'outside pointerdown closes quick settings');
 await gear.dblclick();
 await page.getByRole('heading',{name:'Gjør ELIX til din.'}).waitFor();
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.length),beforeTabs+1);
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.find(t=>t.id==='original').url),'https://example.com/form','settings preserve existing page');
 await gear.dblclick();
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.length),beforeTabs+1,'double click reuses settings tab');
 await page.getByRole('button',{name:'Vis bare ikoner',exact:true}).click();
 await page.getByRole('button',{name:'Utvid sidemeny',exact:true}).waitFor();
 const narrow=await page.locator('.rail').boundingBox();
 await gear.click();await quick.waitFor();
 assert.ok((await page.locator('.rail').boundingBox()).width>narrow.width,'icon sidebar expands without native overlay');
 await page.screenshot({path:join(output,'gear-menu.png'),fullPage:true});
 await page.keyboard.press('Escape');
 assert.equal((await page.locator('.rail').boundingBox()).width,narrow.width,'closing returns to icons');
 await gear.dblclick();
 assert.equal(await quick.count(),0,'compact double click navigates instead of leaving menu open');
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.length),beforeTabs+1);
 await page.getByRole('button',{name:'Utvid sidemeny',exact:true}).click();
 await page.getByRole('tab',{name:'Original page',exact:true}).click();
 console.log('PASS: gear click/double click, accordion, Escape/outside close and compact layout');
 // Drag feedback must survive real pointer capture, cancel cleanly and change order only on drop.
 const strip=id=>page.locator(`.tab-scroll [data-tab-id="${id}"]`);
 const row=id=>page.locator(`.sidebar-tabs [data-tab-id="${id}"]`);
 const grab=async locator=>{const box=await locator.boundingBox();await page.mouse.move(box.x+Math.min(45,box.width/2),box.y+box.height/2);await page.mouse.down();return box};
 let box=await grab(strip('original'));
 await page.waitForFunction(()=>document.querySelector('.tab.held')?.dataset.tabId==='original');
 assert.equal(await page.locator('.tab-drag-preview').count(),0,'held click has highlight without drag ghost');
 await page.mouse.move(box.x+65,box.y+box.height/2,{steps:3});
 await page.locator('.tab-drag-preview').waitFor();
 assert.match(await page.locator('.tab-drag-preview').innerText(),/Original page/);
 await page.keyboard.press('Escape');await page.mouse.up();
 assert.equal(await page.locator('.tab-drag-preview').count(),0,'Escape clears ghost');
 assert.equal(await page.locator('.held,.dragging').count(),0,'Escape clears held state');
 assert.equal(await page.evaluate(()=>window.__uiTest.calls.filter(c=>c.action==='reorder_tab').length),0,'cancel does not reorder');
 box=await grab(strip('original'));
 const target=await strip('third').boundingBox();
 await page.mouse.move(target.x+target.width-8,target.y+target.height/2,{steps:8});
 await page.waitForFunction(()=>document.querySelector('.tab.drop-after')?.dataset.tabId==='third');
 await page.screenshot({path:join(output,'drag-tab.png'),fullPage:true});
 await page.mouse.up();
 await page.waitForFunction(()=>window.__uiTest.state.session.tabs.slice(0,3).map(t=>t.id).join(',')==='peer,third,original');
 assert.equal(await page.locator('.tab-drag-preview').count(),0);
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.active),'original','reorder suppresses target activation');
 box=await grab(row('original'));
 const first=await row('peer').boundingBox();
 await page.mouse.move(first.x+45,first.y+2,{steps:8});
 await page.waitForFunction(()=>document.querySelector('.sidebar-tabs .drop-before')?.dataset.tabId==='peer');
 await page.mouse.up();
 await page.waitForFunction(()=>window.__uiTest.state.session.tabs.slice(0,3).map(t=>t.id).join(',')==='original,peer,third');
 box=await grab(strip('original'));
 await page.mouse.move(box.x+65,box.y+box.height/2,{steps:3});
 await page.locator('.tab-drag-preview').waitFor();
 await page.evaluate(()=>window.dispatchEvent(new Event('blur')));await page.mouse.up();
 assert.equal(await page.locator('.tab-drag-preview').count(),0,'window blur clears feedback');
 assert.equal(await page.evaluate(()=>window.__uiTest.calls.filter(c=>c.action==='reorder_tab').length),2,'window blur does not reorder');

 console.log('PASS: held/drag feedback, Escape/blur cancellation and horizontal/vertical reorder');
 // A collapsed group remains a visible drop target even though its tabs are hidden.
 await gear.click();
 await quick.getByRole('button',{name:'Faner',exact:true}).click();
 await quick.getByLabel('Fanegruppe',{exact:true}).fill('Lesing');
 await quick.getByRole('button',{name:'Lagre fanegruppe',exact:true}).click();
 const groupHeading=page.locator('[data-tab-group-anchor="original"]');
 await groupHeading.waitFor();
 await page.keyboard.press('Escape');
 await groupHeading.click();
 assert.equal(await groupHeading.getAttribute('aria-expanded'),'false');
 await grab(strip('peer'));
 const headingBox=await groupHeading.boundingBox();
 await page.mouse.move(headingBox.x+35,headingBox.y+headingBox.height/2,{steps:8});
 await page.locator('.group-drop-target').waitFor();
 assert.match(await page.locator('.tab-drag-preview').innerText(),/Slipp i gruppen Lesing/);
 await page.mouse.up();
 await page.waitForFunction(()=>window.__uiTest.state.session.tabs.find(t=>t.id==='peer').group==='Lesing');
 assert.equal(await groupHeading.getAttribute('aria-expanded'),'false');
 assert.equal(await row('peer').count(),0,'the group stays collapsed after a drop');
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.active),'original','grouping preserves active tab');
 assert.equal(await page.evaluate(()=>window.__uiTest.calls.filter(c=>c.action==='reorder_tab').length),2,'grouping uses one native write');
 await groupHeading.click();
 await row('peer').waitFor();
 console.log('PASS: drag into collapsed tab group with visible target and membership update');
 // Extract each possible member, including the last member dropped in place beside an ungrouped tab.
 const originalSession=await page.evaluate(()=>structuredClone(window.__uiTest.state.session));
 for(const source of ['original','peer','third']){
  await page.evaluate(session=>{
   const ui=window.__uiTest;
   ui.state.session=structuredClone(session);
   ui.state.session.tabs.slice(0,3).forEach(tab=>{tab.group='Lesing'});
   ui.emit('browser-changed',ui.state.session);
  },originalSession);
  const outside=originalSession.tabs[3].id;
  await row(source).waitFor();
  await grab(row(source));
  const target=await row(outside).boundingBox();
  await page.mouse.move(target.x+45,target.y+2,{steps:8});
  await page.locator('.sidebar-tabs .drop-before').waitFor();
  assert.match(await page.locator('.tab-drag-preview').innerText(),/Ta ut av gruppen/);
  await page.mouse.up();
  await page.waitForFunction(id=>window.__uiTest.state.session.tabs.find(tab=>tab.id===id).group==='',source);
  const snapshot=await page.evaluate(()=>window.__uiTest.state.session);
  assert.deepEqual(snapshot.tabs.slice(0,2).map(tab=>[tab.id,tab.group]),
   ['original','peer','third'].filter(id=>id!==source).map(id=>[id,'Lesing']));
  assert.equal(snapshot.tabs[2].id,source);
  assert.equal(snapshot.active,'original');
 }
 // The dedicated drop area also works when every tab belongs to one group.
 await page.evaluate(()=>{
  const ui=window.__uiTest;
  ui.state.session.tabs.forEach(tab=>{tab.group='Lesing'});
  ui.emit('browser-changed',ui.state.session);
 });
 const activeBox=await grab(strip('original'));
 await page.mouse.move(activeBox.x+65,activeBox.y+activeBox.height/2,{steps:4});
 const zone=page.locator('[data-tab-ungroup]');
 const zoneBox=await zone.boundingBox();
 await page.mouse.move(zoneBox.x+25,zoneBox.y+zoneBox.height/2,{steps:8});
 await page.locator('.tab-ungroup-target.targeted').waitFor();
 await page.mouse.up();
 await page.waitForFunction(()=>window.__uiTest.state.session.tabs.find(t=>t.id==='original').group==='');
 assert.ok(await page.evaluate(()=>window.__uiTest.state.session.tabs.filter(t=>t.id!=='original').every(t=>t.group==='Lesing')));
 console.log('PASS: removing the first, middle or final group member leaves siblings and active tab intact');
 // Bookmarks use the same collection in the page and toolbar; mutations retain profile identity.
 await page.getByRole('button',{name:'Bokmerker',exact:true}).click();
 const library=page.locator('.bookmark-collection-page');
 await library.waitFor();
 await library.getByRole('button',{name:'Bokmerkegruppe Tom gruppe',exact:true}).waitFor();
 assert.match(await library.locator('.bookmark-folder').filter({hasText:'Tom gruppe'}).innerText(),/Gruppen er tom/);
 const workGroup=library.getByRole('button',{name:'Bokmerkegruppe Arbeid',exact:true});
 const workBookmark=library.locator('.bookmark-open').filter({hasText:'Work bookmark'});
 await workGroup.click();
 assert.equal(await workGroup.getAttribute('aria-expanded'),'false');
 await workBookmark.waitFor({state:'hidden'});
 const filter=library.getByRole('textbox',{name:'Søk i bokmerker',exact:true});
 await filter.click({button:'right'});
 assert.equal(await library.getByRole('menu',{name:'Bokmerkevalg'}).count(),0,'text fields keep their own context menu');
 await page.keyboard.press('Escape');
 await filter.fill('Work bookmark');
 await workBookmark.waitFor();
 await filter.fill('');
 assert.equal(await workGroup.getAttribute('aria-expanded'),'false','clearing search restores collapsed groups');
 await workGroup.click({button:'right'});
 await page.waitForFunction(()=>document.activeElement?.textContent==='Gi gruppen nytt navn');
 await page.keyboard.press('Escape');
 await library.getByRole('button',{name:'Ny gruppe',exact:true}).click();
 await library.getByRole('textbox',{name:'Gruppenavn',exact:true}).fill('Gaming');
 await library.getByRole('button',{name:'Lagre',exact:true}).click();
 await library.getByRole('button',{name:'Bokmerkegruppe Gaming',exact:true}).waitFor();
 assert.ok(await page.evaluate(()=>window.__uiTest.libraries['test-profile'].groups.includes('Gaming')),'empty groups persist through list refresh');
 const bookmark=library.locator('.bookmark-open').filter({hasText:'ELIX bookmark'});
 await bookmark.click({button:'right'});
 await library.getByRole('menuitem',{name:'Flytt til gruppe …',exact:true}).click();
 await library.getByRole('combobox',{name:'Flytt til bokmerkegruppe',exact:true}).selectOption('Gaming');
 await library.getByRole('button',{name:'Lagre',exact:true}).click();
 await page.waitForFunction(()=>window.__uiTest.libraries['test-profile'].items.find(i=>i.id===1).group==='Gaming');
 let gaming=library.locator('.bookmark-folder').filter({has:page.getByRole('button',{name:'Bokmerkegruppe Gaming',exact:true})});
 await gaming.locator('.bookmark-open').filter({hasText:'ELIX bookmark'}).waitFor();
 await library.getByRole('button',{name:'Bokmerkegruppe Gaming',exact:true}).click({button:'right'});
 await library.getByRole('menuitem',{name:'Gi gruppen nytt navn',exact:true}).click();
 await library.getByRole('textbox',{name:'Gruppenavn',exact:true}).fill('Spill');
 await library.getByRole('button',{name:'Lagre',exact:true}).click();
 await library.getByRole('button',{name:'Bokmerkegruppe Spill',exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.__uiTest.libraries['test-profile'].items.find(i=>i.id===1).group),'Spill','rename retains bookmark membership');
 await page.screenshot({path:join(output,'bookmark-groups.png'),fullPage:true});
 await library.getByRole('button',{name:'Bokmerkegruppe Spill',exact:true}).click({button:'right'});
 await library.getByRole('menuitem',{name:'Fjern gruppe – behold bokmerker',exact:true}).click();
 await page.waitForFunction(()=>!window.__uiTest.libraries['test-profile'].groups.includes('Spill'));
 await library.getByRole('button',{name:'Bokmerkegruppe Spill',exact:true}).waitFor({state:'hidden'});
 assert.equal(await page.evaluate(()=>window.__uiTest.libraries['test-profile'].items.length),2,'removing group preserves all bookmarks');
 assert.equal(await page.evaluate(()=>window.__uiTest.libraries['test-profile'].items.find(i=>i.id===1).group),'');
 await bookmark.focus();await page.keyboard.press('Shift+F10');
 await library.getByRole('menu',{name:'Bokmerkevalg',exact:true}).waitFor();

 // Focus the address as soon as the editor appears, before the next paint.
 // Delayed initial autofocus must not redirect the user's next paste to the title.
 await page.evaluate(()=>{
  window.__bookmarkAddressFocus = new Promise(resolve=>{
   const observer = new MutationObserver(()=>{
    const input = document.querySelector('input[aria-label="Bokmerkeadresse"]');
    if (!input) return;
    observer.disconnect();
    input.focus();
    requestAnimationFrame(()=>requestAnimationFrame(()=>resolve(document.activeElement===input)));
   });
   observer.observe(document.body,{childList:true,subtree:true});
  });
 });
 await library.getByRole('menuitem',{name:'Rediger bokmerke',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.__bookmarkAddressFocus),true,'opening the editor must preserve a newer focus choice');
 await library.getByRole('textbox',{name:'Bokmerketittel',exact:true}).fill('ELIX saved');
 await library.getByRole('textbox',{name:'Bokmerkeadresse',exact:true}).fill('https://projectelix.no/');
 await library.getByRole('button',{name:'Lagre',exact:true}).click();
 const edited=library.locator('.bookmark-open').filter({hasText:'ELIX saved'});
 await edited.waitFor();
 assert.deepEqual(await page.evaluate(()=>{const {title,url}=window.__uiTest.libraries['test-profile'].items.find(item=>item.id===1);return {title,url};}),{title:'ELIX saved',url:'https://projectelix.no/'});
 await edited.click({button:'right'});
 const countBeforeOpen=await page.evaluate(()=>window.__uiTest.state.session.tabs.length);
 await library.getByRole('menuitem',{name:'Åpne i ny fane',exact:true}).click();
 await page.waitForFunction(n=>window.__uiTest.state.session.tabs.length===n+1,countBeforeOpen);
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.at(-1).url),'https://projectelix.no/');
 assert.equal(await page.evaluate(()=>window.__uiTest.state.session.tabs.find(t=>t.id==='original').url),'https://example.com/form');
 await page.locator('.native-view').waitFor();
 // The toolbar context panel pushes native content down and stays inside shell bounds.
 const toolbar=page.locator('.bookmark-collection-toolbar');
 await toolbar.locator('.bookmark-open').filter({hasText:'ELIX saved'}).click({button:'right'});
 await toolbar.getByRole('menu',{name:'Bokmerkevalg',exact:true}).waitFor();
 const actionsBox=await toolbar.locator('.bookmark-actions').boundingBox(),nativeBox=await page.locator('.native-view').boundingBox();
 assert.ok(actionsBox.y+actionsBox.height<=nativeBox.y+1,'bookmark menu stays above native webpage');
 await page.keyboard.press('Escape');
 await toolbar.getByRole('menu',{name:'Bokmerkevalg',exact:true}).waitFor({state:'hidden'});
 assert.equal(await toolbar.getByRole('menu',{name:'Bokmerkevalg',exact:true}).count(),0,'Escape closes bookmark menu');
 // Profile UI refresh must never reuse the previous profile bookmarks/groups.
 await page.locator('button.profile-avatar[title="Profiler"]').click();
 await page.locator('.profile-card').filter({has:page.getByText('Andre',{exact:true})}).click();
 await page.waitForFunction(()=>window.__uiTest.state.profile==='other-profile');
 await page.locator('.profile-card.current').filter({has:page.getByText('Andre',{exact:true})}).waitFor();
 await page.getByRole('button',{name:'Bokmerker',exact:true}).click();
 await library.locator('.bookmark-open').filter({hasText:'Other profile bookmark'}).waitFor();
 assert.equal(await library.locator('.bookmark-open').filter({hasText:'ELIX saved'}).count(),0);
 assert.equal(await library.getByRole('button',{name:'Bokmerkegruppe Arbeid',exact:true}).count(),0);
 await library.getByRole('button',{name:'Bokmerkegruppe Andre ting',exact:true}).waitFor();
 assert.equal(await library.getByRole('menu',{name:'Bokmerkevalg',exact:true}).count(),0);
 assert.deepEqual(await page.evaluate(()=>window.__uiTest.calls.filter(c=>['create_bookmark_group','rename_bookmark_group','remove_bookmark_group','move_bookmark','edit_bookmark'].includes(c.action)).map(c=>c.profile)),Array(5).fill('test-profile'));


 assert.deepEqual(errors,[]);
 console.log('PASS: bookmark CRUD/group/move, keyboard context, native content bounds and profile isolation');
 console.log('PASS Alpha 8 shell UI; native behavior is mocked.');
} catch(error) {
 await page.screenshot({path:join(output,'failure.png'),fullPage:true});
 console.error(await page.locator('body').innerText());
 throw error;
} finally {await browser.close();await server.close()}
