// DOM/CSS checks complement the real WebView2 reparenting/stream tests in smoke.rs.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chromium} from 'playwright';
const script = await readFile(new URL('../src-tauri/src/browser/media-presentation.js', import.meta.url), 'utf8');
const mediaSessionScript = await readFile(new URL('../src-tauri/src/browser/media-session.js', import.meta.url), 'utf8');
const browser = await chromium.launch({headless:true, ...(process.env.CHROMIUM_PATH ? {executablePath:process.env.CHROMIUM_PATH}: {}), args:['--no-sandbox']});
const page = await browser.newPage({viewport:{width:720,height:440}});
const errors=[]; page.on('pageerror',error=>errors.push(error.message));
const run=()=>page.evaluate(script);
const restore=()=>page.evaluate(()=>window.__elixMediaPresentation.restore());
const makeMedia=async(mode='video')=>page.evaluate(mode=>{
  const host = document.querySelector('#host');
  let root = document;
  if (mode === 'frame') root = document.querySelector('iframe').contentDocument;
  if (mode === 'shadow') root = host.attachShadow({mode:'open'});
  if (mode === 'slotted') {
    const clip = document.createElement('div');
    clip.style.cssText = 'transform:translate(16px);overflow:hidden;contain:paint;width:140px;height:95px';
    clip.append(document.createElement('slot'));
    host.attachShadow({mode:'open'}).append(clip);
    root = host;
  }
  const doc=root.ownerDocument||root;
  const wrap=doc.createElement('section');
  wrap.id='wrapper'; wrap.style.cssText='transform:translate(7px);contain:paint;overflow-x:hidden;margin-left:13px;border-left:2px solid red;background-color:rgb(3, 5, 8);position:sticky;width:130px;height:90px;';
  const media=doc.createElement(mode==='audio'?'audio':'video');
  media.style.cssText='width:120px;height:80px;background-color:rgb(11, 13, 17);margin-left:5px;border-left:1px solid red;';
  media.muted=true;media.playsInline=true;
  const canvas=doc.createElement('canvas');canvas.width=320;canvas.height=180;
  canvas.getContext('2d').fillRect(0,0,320,180);
  const stream=canvas.captureStream(20);media.srcObject=stream;
  wrap.appendChild(media);(root.body||root).appendChild(wrap);
  const snapshot=el=>Object.fromEntries(Array.from(el.style,p=>[p,[el.style.getPropertyValue(p),el.style.getPropertyPriority(p)]]));
  window.__fixture={media,wrap,stream,snapshot,mediaStyle:snapshot(media),wrapStyle:snapshot(wrap),canvas};
  if(mode!=='audio') {setInterval(()=>canvas.getContext('2d').fillRect(0,0,320,180),40);void media.play();}
},mode);
try {
  await page.setContent('<title>No media</title><p>Ordinary page</p>');
  assert.deepEqual(await run(),{ok:false,reason:'no-media'});
  assert.equal(await page.evaluate(()=>!!document.querySelector('[data-elix-media-surface]')),false);

  await page.setContent('<title>Video</title><div id="chrome">Menus, ads and comments</div>');
  await makeMedia();
  await page.waitForFunction(()=>!window.__fixture.media.paused);
  // A second loaded but paused video must not win over the playing element.
  await page.evaluate(()=>{const v=document.createElement('video');v.srcObject=window.__fixture.stream;v.style.cssText='width:2000px;height:1000px';document.body.appendChild(v)});
  assert.deepEqual(await run(),{ok:true,kind:'video'});
  const fills=()=>page.evaluate(()=>{const v=window.__fixture.media,r=v.getBoundingClientRect();return Math.abs(r.x)<1&&Math.abs(r.y)<1&&Math.abs(r.width-innerWidth)<1&&Math.abs(r.height-innerHeight)<1&&v.srcObject===window.__fixture.stream&&!v.paused&&getComputedStyle(document.querySelector('#chrome')).visibility==='hidden'});
  assert.equal(await fills(),true,'Playing video alone fills the viewport through sticky/clipped ancestors');
  await page.setViewportSize({width:460,height:300});
  assert.equal(await fills(),true,'Video follows resized popout');
  // Exercise the Chromium UA media controls, not a replacement JS play button.
  await page.mouse.move(30,280);
  await page.mouse.click(30,274);
  await page.waitForFunction(()=>window.__fixture.media.paused,null,{timeout:5000});
  await page.mouse.click(30,274);
  await page.waitForFunction(()=>!window.__fixture.media.paused,null,{timeout:5000});
  await restore();
  assert.deepEqual(await page.evaluate(()=>{const f=window.__fixture;return {media:f.snapshot(f.media),wrapper:f.snapshot(f.wrap)}}),await page.evaluate(()=>({media:window.__fixture.mediaStyle,wrapper:window.__fixture.wrapStyle})),'Restoration preserves all original CSS longhands and normalized colors');
  assert.equal(await page.evaluate(()=>window.__fixture.media.srcObject===window.__fixture.stream&&!window.__fixture.media.controls&&!window.__fixture.media.paused&&!window.__elixMediaPresentation&&getComputedStyle(document.querySelector('#chrome')).visibility==='visible'),true);

  for(const mode of ['audio','shadow','slotted','frame']) {
    await page.setContent(`<title>${mode} fixture</title><div id="chrome">Site content</div><div id="host" style="transform:translate(9px);contain:paint"></div>${mode==='frame'?'<iframe style="width:180px;height:100px" srcdoc="<!doctype html><title>Embedded video</title><p>Frame chrome</p>"></iframe>':''}`);
    if(mode==='frame')await page.waitForFunction(()=>document.querySelector('iframe').contentDocument?.querySelector('p'));
    await makeMedia(mode);
    const clipStyle = mode === 'slotted' ? await page.evaluate(() => window.__fixture.snapshot(document.querySelector('#host').shadowRoot.firstElementChild)) : null;
    assert.deepEqual(await run(),{ok:true,kind:mode==='audio'?'audio':'video'});
    assert.equal(await page.evaluate(mode=>{const v=window.__fixture.media,r=v.getBoundingClientRect(),w=v.ownerDocument.defaultView;return v.controls&&getComputedStyle(v).visibility==='visible'&&v.srcObject===window.__fixture.stream&&(mode==='audio'?r.width>innerWidth-60&&r.height===54:Math.abs(r.width-w.innerWidth)<1&&Math.abs(r.height-w.innerHeight)<1)},mode),true,`${mode} exposes only the original media player`);
    if(mode==='frame')assert.equal(await page.evaluate(()=>{const r=document.querySelector('iframe').getBoundingClientRect();return Math.abs(r.x)<1&&Math.abs(r.y)<1&&Math.abs(r.width-innerWidth)<1&&Math.abs(r.height-innerHeight)<1}),true,'Same-origin iframe fills the outer viewport');
    if (mode === 'slotted') {
      await page.setViewportSize({width:600,height:380});
      assert.equal(await page.evaluate(() => {
        const video = window.__fixture.media;
        const {x,y,width,height} = video.getBoundingClientRect();
        return x === 0 && y === 0 && width === innerWidth && height === innerHeight
          && document.elementFromPoint(innerWidth - 10, 10) === video;
      }), true, 'Slotted video stays visible and clickable up to the edge after resizing');
    }
    if(mode==='frame') {
      await page.evaluate(()=>{document.querySelector('iframe').srcdoc='<h1>Destination must stay hidden</h1>'});
      await page.waitForFunction(()=>document.querySelector('iframe').contentDocument?.querySelector('h1'));
      await page.waitForFunction(()=>getComputedStyle(document.querySelector('iframe')).visibility==='hidden');
    }
    await restore();
    if (mode === 'slotted') {
      assert.deepEqual(await page.evaluate(() => window.__fixture.snapshot(document.querySelector('#host').shadowRoot.firstElementChild)), clipStyle, 'Original slot wrapper restored');
    }
    assert.deepEqual(await page.evaluate(()=>{const f=window.__fixture;return {media:f.snapshot(f.media),wrapper:f.snapshot(f.wrap)}}),await page.evaluate(()=>({media:window.__fixture.mediaStyle,wrapper:window.__fixture.wrapStyle})),`${mode} original styles restored`);
    assert.equal(await page.evaluate(()=>window.__fixture.media.srcObject===window.__fixture.stream&&!window.__fixture.media.controls&&!document.querySelector('[data-elix-media-surface]')),true);
  }
  // A real secure-context Media Session, registered before ELIX opens the player.
  await page.addInitScript({content: mediaSessionScript});
  await page.route('https://playlist.test/', route => route.fulfill({contentType:'text/html', body:'<title>Playlist</title>'}));
  await page.goto('https://playlist.test/');
  await makeMedia();
  await page.waitForFunction(()=>!window.__fixture.media.paused);
  await page.evaluate(()=>{
    window.trackLog = [];
    navigator.mediaSession.setActionHandler('nexttrack', details => window.trackLog.push(details.action));
  });
  await page.evaluate(()=>{window.__fixture.media.volume = .3});
  await run();
  const volume = page.getByRole('slider',{name:'Volum i minispiller'});
  assert.equal(await volume.inputValue(), '30', 'Starts at the website volume');
  await page.evaluate(()=>{window.__fixture.media.volume = .4});
  await page.waitForFunction(()=>document.querySelector('[data-elix-track-controls]').shadowRoot.querySelector('input').value === '40');
  // Exercise the visible, keyboard-accessible control as the user does.
  await volume.press('End');
  await volume.press('ArrowLeft');
  assert.equal(await volume.inputValue(), '99');
  await page.evaluate(()=>{window.__fixture.media.volume = .15});
  await page.waitForFunction(()=>window.__fixture.media.volume === .99);
  assert.equal(await volume.inputValue(),'99','Site writes cannot reset the explicit popout level');
  assert.equal(await page.evaluate(()=>window.__fixture.media.muted),true,'Volume does not toggle mute');
  const next = page.getByRole('button',{name:'Neste spor'});
  const prev = page.getByRole('button',{name:'Forrige spor'});
  assert.equal(await prev.isVisible(),false);
  await next.click();
  assert.deepEqual(await page.evaluate(()=>window.trackLog),['nexttrack']);
  await page.evaluate(()=>{
    navigator.mediaSession.setActionHandler('nexttrack',null);
    navigator.mediaSession.setActionHandler('previoustrack', details=>window.trackLog.push(details.action));
  });
  await next.waitFor({state:'hidden'});
  await prev.click();
  assert.deepEqual(await page.evaluate(()=>window.trackLog),['nexttrack','previoustrack']);
  assert.equal(await page.evaluate(()=>document.querySelectorAll('video').length===1&&!window.__fixture.media.paused),true);
  // A site may swap the DOM node between songs. It owns the new play state.
  await page.evaluate(()=>{
    const fixture = window.__fixture;
    fixture.media.pause();
    const replacement = document.createElement('video');
    replacement.muted=true;
    replacement.volume=.2;
    replacement.srcObject=fixture.stream;
    fixture.media.replaceWith(replacement);
    window.nextMedia=replacement;
  });
  await page.waitForFunction(()=>window.nextMedia.controls);
  assert.equal(await page.evaluate(()=>window.nextMedia.paused && !window.__fixture.media.controls),true);
  assert.equal(await volume.inputValue(),'99','The active volume follows a replacement track');
  assert.equal(await page.evaluate(()=>window.nextMedia.volume),.99);
  await restore();
  assert.equal(await volume.count(),0);
  await page.evaluate(()=>{window.nextMedia.volume = .2});
  await page.waitForFunction(()=>window.nextMedia.volume === .2 && !window.nextMedia.hasAttribute('data-elix-volume-control'));
  assert.equal(await page.evaluate(()=>!window.nextMedia.controls && !document.querySelector('[data-elix-track-controls]')),true);
  assert.deepEqual(errors,[]);
  console.log('PASS: media-only video/audio, native Chromium play/pause controls, resize, same-origin iframe and shadow DOM, stream identity, playlist actions, explicit popout volume, media replacement, no-media rejection and CSS restoration. Native hosting remains covered by Windows smoke tests.');
} finally {await browser.close();}
