import test from 'node:test';
import assert from 'node:assert/strict';
import {parseEnv} from 'node:util';
import {mappedMail,envText,originOf,nginxSnippet,addNginxInclude,serviceText,assertFirstInstall,verifyPublic,assertReusableSnippet} from './install-smtp.mjs';

test('existing SMTP mapping excludes unrelated application secrets and respects final overrides',()=>{
  const source={APP_SMTP_HOST:'smtp.example.com',APP_SMTP_PORT:'587',APP_SMTP_USER:'tester',APP_SMTP_PASSWORD:'synthetic-password',APP_MAIL_FROM:'no-reply@example.com',STRIPE_SECRET_KEY:'excluded',DATABASE_PATH:'excluded'};
  const mapped=mappedMail(source);
  assert.equal(mapped.SMTP_PASSWORD,'synthetic-password');
  assert.equal(mapped.BREVO_SENDER_EMAIL,'no-reply@example.com');
  assert.equal(mapped.STRIPE_SECRET_KEY,undefined);
  assert.equal(mapped.DATABASE_PATH,undefined);
  assert.deepEqual(parseEnv(envText(mapped)),mapped);
  assert.throws(()=>mappedMail({...source,APP_SMTP_PASSWORD:'bad\nvalue'}));
  assert.throws(()=>mappedMail({...source,APP_SMTP_PORT:'25'}));
});
const original=`server {\n    listen 80;\n    server_name member.example.com;\n    return 301 https://member.example.com$request_uri;\n}\nserver {\n    listen 443 ssl;\n    server_name member.example.com;\n    client_max_body_size 64k;\n    location / {\n        proxy_pass http://127.0.0.1:4190;\n    }\n}\n`;
test('HTTPS include preserves the existing membership server and rejects ambiguous layouts',()=>{
  const updated=addNginxInclude(original,'https://member.example.com');
  assert.equal(updated.replace('\n    include /etc/nginx/snippets/elix-accounts.conf;',''),original);
  assert.throws(()=>addNginxInclude(original,'https://wrong.example.com'));
  assert.throws(()=>addNginxInclude(original+original,'https://member.example.com'));
  assert.throws(()=>addNginxInclude(updated,'https://member.example.com'));
  assert.throws(()=>addNginxInclude(original.replace('location / {','location /v1/ {'),'https://member.example.com'));
});
test('proxy overwrites forwarding headers, bounds bodies and isolates explicit routes',()=>{
  const snippet=nginxSnippet();
  assert.equal((snippet.match(/proxy_set_header X-Forwarded-For \$remote_addr;/g)||[]).length,5);
  assert.equal((snippet.match(/access_log off;/g)||[]).length,5);
  assert.equal((snippet.match(/client_max_body_size 6m;/g)||[]).length,5);
  assert.ok(snippet.includes('location = /elix-accounts-health'));
  assert.ok(snippet.includes('proxy_pass http://127.0.0.1:3210/health;'));
  assert.ok(!snippet.includes('location / {'));
});
test('origin and service executable cannot inject configuration',()=>{
  assert.throws(()=>originOf('http://example.com'));
  assert.throws(()=>originOf('https://example.com/private'));
  assert.throws(()=>serviceText('/usr/bin/node\nUser=root'));
  const unit=serviceText('/opt/runtime/bin/node');
  assert.ok(unit.includes('User=elix-accounts\n'));
  assert.ok(unit.includes('StateDirectoryMode=0700'));
  assert.ok(unit.includes('ProtectSystem=strict'));
  assert.ok(unit.includes('UMask=0077'));
});
test('first install refuses every partial-install artifact and reused service identity',()=>{
  const empty={portFree:true,environment:false,unit:false,current:false,data:false,user:false,group:false};
  assert.doesNotThrow(()=>assertFirstInstall(empty));
  assert.throws(()=>assertFirstInstall({...empty,portFree:false}));
  for(const field of ['environment','unit','current','data','user','group'])assert.throws(()=>assertFirstInstall({...empty,[field]:true}),field);
});
test('public verification waits for new workers after stale 404 responses',async()=>{
  let clock=0;const calls=[];
  const result=await verifyPublic('https://member.example.com',{
    now:()=>clock,sleep:async ms=>{clock+=ms},
    send:async(url,options)=>{calls.push({url,options});return clock===0?{status:404,body:'old membership response'}:{status:200,body:JSON.stringify(url.endsWith('/v1/config')?{email:true}:{ok:true})}},
  });
  assert.deepEqual(result,{ok:true,healthStatus:200,configStatus:200});
  assert.equal(clock,250);assert.equal(calls.length,4);
  assert.ok(calls.every(call=>call.options.timeoutMs<=2000));
});
test('public verification stops after ten seconds and returns only statuses on failure',async()=>{
  let clock=0;
  const result=await verifyPublic('https://member.example.com',{
    now:()=>clock,sleep:async ms=>{clock+=ms},
    send:async url=>{if(url.endsWith('/v1/config'))throw new Error('private diagnostic');return {status:502,body:'private upstream response'}},
  });
  assert.equal(clock,10000);assert.deepEqual(result,{ok:false,healthStatus:502,configStatus:null});
  assert.doesNotMatch(JSON.stringify(result),/private/);
});
test('publication retry reuses only the exact root-owned protected regular snippet',()=>{
  const stat={isFile:()=>true,uid:0,mode:0o100644},contents=nginxSnippet();
  assert.doesNotThrow(()=>assertReusableSnippet(stat,contents));
  assert.throws(()=>assertReusableSnippet({...stat,isFile:()=>false},contents));
  assert.throws(()=>assertReusableSnippet({...stat,uid:1000},contents));
  assert.throws(()=>assertReusableSnippet({...stat,mode:0o100664},contents));
  assert.throws(()=>assertReusableSnippet(stat,contents+'# changed\n'));
});
