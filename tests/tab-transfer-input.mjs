import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chromium} from 'playwright';

const directory=new URL('../src-tauri/src/browser/',import.meta.url);
const detector=await readFile(new URL('transfer-input.js',directory),'utf8');
const check=await readFile(new URL('transfer-input-check.js',directory),'utf8');
const browser=await chromium.launch({headless:true,...(process.env.CHROMIUM_PATH?{executablePath:process.env.CHROMIUM_PATH}:{})});
try {
  const context=await browser.newContext();
  await context.addInitScript({content:detector});
  await context.route('https://*.test/**',route=>{
    const url=new URL(route.request().url());
    const html=url.pathname==='/frame'?'<iframe src="/edit"></iframe>'
      :url.pathname==='/unknown'?'<iframe src="https://embed.test/edit"></iframe>'
      :'<form action="/done"><textarea aria-label="Kommentar"></textarea><button>Send</button></form>';
    return route.fulfill({contentType:'text/html',body:html});
  });
  const page=await context.newPage();
  await page.goto('https://source.test/edit');
  assert.equal(await page.evaluate(check),'clean');
  await page.getByRole('textbox').fill('utkast');
  assert.equal(await page.evaluate(check),'edited');
  // An AJAX or rejected form submission cannot prove that the data was saved.
  await page.evaluate(()=>document.querySelector('form').addEventListener('submit',e=>e.preventDefault(),{once:true}));
  await page.getByRole('button').click();
  assert.equal(await page.evaluate(check),'edited');
  await Promise.all([page.waitForURL('**/done*'),page.getByRole('button').click()]);
  assert.equal(await page.evaluate(check),'clean');
  await page.goto('https://source.test/frame');
  assert.equal(await page.evaluate(check),'clean');
  await page.frameLocator('iframe').getByRole('textbox').pressSequentially('nytt');
  assert.equal(await page.evaluate(check),'edited');
  await page.goto('https://source.test/unknown');
  assert.equal(await page.evaluate(check),'unknown');
  console.log('PASS: tab transfer preserves typed forms, cancelled submits and embedded drafts. A new document starts clean and inaccessible frames keep their source tab.');
} finally {await browser.close()}
