import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtemp,rm,symlink} from 'node:fs/promises';
import {createServer as createSocketServer} from 'node:net';
import {tmpdir} from 'node:os';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createAccounts} from './server.mjs';
for(const preserveMain of [false,true])test(`CLI starts through the current directory symlink${preserveMain?' with preserved main symlinks':''}`,async()=>{
 const directory=await mkdtemp(join(tmpdir(),'elix-accounts-startup-'));
 let child,closed;
 try{
  await symlink(dirname(fileURLToPath(import.meta.url)),join(directory,'current'),process.platform==='win32'?'junction':'dir');
  const socket=createSocketServer();await new Promise(resolve=>socket.listen(0,'127.0.0.1',resolve));
  const port=socket.address().port;await new Promise(resolve=>socket.close(resolve));
  child=spawn(process.execPath,[...(preserveMain?['--preserve-symlinks-main']:[]),join(directory,'current','server.mjs')],{
   env:{NODE_ENV:'development',PUBLIC_URL:'http://127.0.0.1',AUTH_PEPPER:'startup-test-pepper-with-at-least-32-characters',DATABASE_PATH:join(directory,'accounts.sqlite'),PORT:String(port)},
   stdio:['ignore','pipe','pipe'],
  });
  closed=new Promise(resolve=>child.once('close',(code,signal)=>resolve({code,signal})));
  let output='';child.stdout.on('data',chunk=>{output+=chunk});child.stderr.on('data',chunk=>{output+=chunk});
  await new Promise((resolve,reject)=>{
   const timer=setTimeout(()=>finish(new Error(`Startup timed out: ${output}`)),5000);
   const finish=error=>{clearTimeout(timer);child.stdout.off('data',ready);child.off('exit',exited);child.off('error',finish);error?reject(error):resolve()};
   const ready=()=>{if(output.includes('ELIX accounts ready'))finish()};
   const exited=(code,signal)=>finish(new Error(`Exited before ready (${code??signal}): ${output}`));
   child.stdout.on('data',ready);child.once('exit',exited);child.once('error',finish);
  });
  const response=await fetch(`http://127.0.0.1:${port}/health`,{signal:AbortSignal.timeout(3000)});
  assert.equal(response.status,200);assert.deepEqual(await response.json(),{ok:true});
  child.kill('SIGTERM');
  const deadline=setTimeout(()=>child.kill('SIGKILL'),3000);
  try{const exit=await closed;if(process.platform!=='win32')assert.deepEqual(exit,{code:0,signal:null});else assert.notEqual(exit.signal,'SIGKILL')}finally{clearTimeout(deadline)}
 }finally{
  if(child&&child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await closed}
  await rm(directory,{recursive:true,force:true});
 }
});
const fixture=async(overrides={},settings={})=>{
 const emails=[];let calls=0;
 const app=createAccounts({publicUrl:'http://127.0.0.1',development:true,database:':memory:',pepper:'test-only-pepper-not-for-production-123',...settings},{sendEmail:async(email,code)=>emails.push({email,code}),discordIdentity:async()=>{calls++;return {id:'123456789012345678',name:'Discord tester'}},...overrides});
 await new Promise(r=>app.server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${app.server.address().port}`;
 const request=async(path,body,token,method=body?'POST':'GET',headers={})=>{const response=await fetch(origin+path,{method,headers:{...(body?{'content-type':'application/json'}:{}),...(token?{authorization:'Bearer '+token}:{}),...headers},body:body?JSON.stringify(body):undefined,redirect:'manual'});let value;try{value=await response.json()}catch{}return {status:response.status,value,response}};
 const login=async email=>{const started=await request('/v1/auth/email/start',{email});assert.equal(started.status,200);const result=await request('/v1/auth/email/verify',{challenge:started.value.challenge,code:emails.at(-1).code});assert.equal(result.status,200);return result.value};
 return {...app,request,emails,login,get calls(){return calls}};
};
test('email code is one-use, five attempts maximum and tokens revoke',async()=>{
 const f=await fixture();try{
 const start=await f.request('/v1/auth/email/start',{email:'person@example.com'});const code=f.emails.at(-1).code;
 const login=await f.request('/v1/auth/email/verify',{challenge:start.value.challenge,code});assert.equal(login.status,200);assert.equal(login.value.account.provider,'email');
 assert.equal((await f.request('/v1/auth/email/verify',{challenge:start.value.challenge,code})).status,401);
 assert.equal((await f.request('/v1/account',null,login.value.token)).status,200);
 await f.request('/v1/auth/logout',{},login.value.token);assert.equal((await f.request('/v1/account',null,login.value.token)).status,401);
 const second=await f.request('/v1/auth/email/start',{email:'person@example.com'});for(let i=0;i<5;i++)assert.equal((await f.request('/v1/auth/email/verify',{challenge:second.value.challenge,code:'00000000'})).status,401);
 assert.equal((await f.request('/v1/auth/email/verify',{challenge:second.value.challenge,code:f.emails.at(-1).code})).status,401);
 assert.equal(f.db.prepare('SELECT count(*) AS n FROM sessions').get().n,0);
 }finally{await f.close()}
});
test('sync is account-isolated, encrypted-envelope only, and detects concurrent edits',async()=>{
 const f=await fixture();try{
 const a=await f.login('a@example.com'),b=await f.login('b@example.com');
 const encrypted={version:1,algorithm:'AES-GCM',iterations:600000,salt:Buffer.alloc(16).toString('base64'),iv:Buffer.alloc(12).toString('base64'),ciphertext:Buffer.alloc(50).toString('base64')};
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted},null,'PUT')).status,401);
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted},a.token,'PUT')).status,200);
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted},a.token,'PUT')).status,409);
 assert.equal((await f.request('/v1/sync',null,b.token)).value.envelope,null);
 assert.equal((await f.request('/v1/sync',{revision:1,envelope:{items:['plain']}},a.token,'PUT')).status,400);
 assert.equal((await f.request('/v1/sync',{revision:1,envelope:encrypted},a.token,'PUT')).value.revision,2);
 assert.equal((await f.request('/v1/sync',{revision:2},a.token,'DELETE')).status,200);
 assert.deepEqual((await f.request('/v1/sync',null,a.token)).value,{revision:3,envelope:null});
 }finally{await f.close()}
});
test('email relogin keeps the account and encrypted copy while device sessions revoke independently',async()=>{
 const f=await fixture();try{
 const first=await f.login('Person@EXAMPLE.com'),second=await f.login('person@example.com');
 assert.equal(first.account.id,second.account.id);assert.notEqual(first.token,second.token);
 const encrypted={version:1,algorithm:'AES-GCM',iterations:600000,salt:Buffer.alloc(16).toString('base64'),iv:Buffer.alloc(12).toString('base64'),ciphertext:Buffer.alloc(50,7).toString('base64')};
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted},first.token,'PUT')).status,200);
 assert.equal((await f.request('/v1/auth/logout',{},first.token)).status,200);
 assert.equal((await f.request('/v1/account',null,first.token)).status,401);
 assert.equal((await f.request('/v1/account',null,second.token)).status,200);
 const relogged=await f.login('person@example.com');
 assert.equal(relogged.account.id,first.account.id);
 assert.deepEqual((await f.request('/v1/sync',null,relogged.token)).value,{revision:1,envelope:encrypted});
 f.db.exec('UPDATE sessions SET expires=0');
 assert.equal((await f.request('/v1/sync',null,relogged.token)).status,401);
 const refreshed=await f.login('person@example.com');
 assert.equal(refreshed.account.id,first.account.id);
 assert.deepEqual((await f.request('/v1/sync',null,refreshed.token)).value,{revision:1,envelope:encrypted});
 }finally{await f.close()}
});
test('sync deletion keeps monotonic revisions and rejects stale uploads and deletes',async()=>{
 const f=await fixture();try{
 const a=await f.login('a@example.com');
 const encrypted=n=>({version:1,algorithm:'AES-GCM',iterations:600000,salt:Buffer.alloc(16).toString('base64'),iv:Buffer.alloc(12).toString('base64'),ciphertext:Buffer.alloc(50,n).toString('base64')});
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted(1)},a.token,'PUT')).value.revision,1);
 assert.equal((await f.request('/v1/sync',{revision:1},a.token,'DELETE')).value.revision,2);
 assert.deepEqual((await f.request('/v1/sync',null,a.token)).value,{revision:2,envelope:null});
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted(2)},a.token,'PUT')).status,409);
 assert.equal((await f.request('/v1/sync',{revision:2,envelope:encrypted(2)},a.token,'PUT')).value.revision,3);
 assert.equal((await f.request('/v1/sync',{revision:1,envelope:encrypted(1)},a.token,'PUT')).status,409);
 assert.equal((await f.request('/v1/sync',{revision:2},a.token,'DELETE')).status,409);
 assert.equal((await f.request('/v1/sync',{},a.token,'DELETE')).status,400);
 assert.deepEqual((await f.request('/v1/sync',null,a.token)).value,{revision:3,envelope:encrypted(2)});
 const b=await f.login('b@example.com');
 assert.equal((await f.request('/v1/sync',{revision:0},b.token,'DELETE')).value.revision,1);
 assert.equal((await f.request('/v1/sync',{revision:0,envelope:encrypted(1)},b.token,'PUT')).status,409);
 assert.equal((await f.request('/v1/sync',{revision:1,envelope:encrypted(1)},b.token,'PUT')).value.revision,2);
 }finally{await f.close()}
});
test('Discord stays unavailable when provider credentials are absent',async()=>{
 const f=await fixture();try{
 assert.equal((await f.request('/v1/config')).value.discord,false);
 assert.equal((await f.request('/v1/config')).value.email,true);
 for(const [path,body] of [['start',{}],['authorize?request=forwarded-challenge',null],['callback?code=victim-code&state=forwarded-state&iss=https://discord.com',null],['complete',{challenge:'forwarded-challenge'}]]){
  assert.equal((await f.request('/v1/auth/discord/'+path,body)).status,503);
 }
 assert.equal(f.calls,0);
 assert.equal(f.db.prepare('SELECT count(*) AS n FROM sessions').get().n,0);
 }finally{await f.close()}
});
test('expired email challenges and foreign browser origins are rejected',async()=>{
 const f=await fixture();try{
 const start=await f.request('/v1/auth/email/start',{email:'a@example.com'});f.db.exec('UPDATE challenges SET expires=0');
 assert.equal((await f.request('/v1/auth/email/verify',{challenge:start.value.challenge,code:f.emails.at(-1).code})).status,401);
 assert.equal((await f.request('/v1/auth/email/start',{email:'b@example.com'},null,'POST',{origin:'https://evil.example'})).status,403);
 }finally{await f.close()}
});
test('failed email delivery deletes the challenge and never creates an account or session',async()=>{
 const f=await fixture({sendEmail:async()=>{throw new Error('Provider detail that must not escape')}});try{
 const result=await f.request('/v1/auth/email/start',{email:'person@example.com'});
 assert.equal(result.status,500);
 assert.equal(JSON.stringify(result.value).includes('Provider detail'),false);
 for(const table of ['challenges','users','sessions'])assert.equal(f.db.prepare(`SELECT count(*) AS n FROM ${table}`).get().n,0);
 }finally{await f.close()}
});
test('email spelling is normalized, login responses never contain the code, and email flood limits apply',async()=>{
 const f=await fixture();try{
 const first=await f.request('/v1/auth/email/start',{email:'  Person@EXAMPLE.com  '});
 assert.equal(first.status,200);assert.equal(f.emails[0].email,'person@example.com');
 assert.equal(JSON.stringify(first.value).includes(f.emails[0].code),false);
 const challenge=f.db.prepare('SELECT subject,code FROM challenges').get();
 assert.equal(challenge.subject,'person@example.com');assert.notEqual(challenge.code,f.emails[0].code);
 for(let i=0;i<4;i++)assert.equal((await f.request('/v1/auth/email/start',{email:'person@example.com'})).status,200);
 assert.equal((await f.request('/v1/auth/email/start',{email:'PERSON@example.com'})).status,429);
 assert.equal(f.emails.length,5);
 }finally{await f.close()}
});

test('verified password enrollment preserves the account, protects old code routes and supports recovery',async()=>{
 const f=await fixture(),email='member@example.com',original='first long password for test',replacement='replacement phrase for test';
 try{
  const legacy=await f.login(email);
  const envelope={version:1,algorithm:'AES-GCM',iterations:600000,salt:Buffer.alloc(16).toString('base64'),iv:Buffer.alloc(12).toString('base64'),ciphertext:Buffer.alloc(20).toString('base64')};
  await f.request('/v1/sync',{revision:0,envelope},legacy.token,'PUT');
  const start=async()=>{const result=await f.request('/v1/auth/password/start',{email});assert.equal(result.status,200);return {challenge:result.value.challenge,code:f.emails.at(-1).code};};
  const verify=(pending,password)=>f.request('/v1/auth/password/verify',{...pending,password});
  const login=password=>f.request('/v1/auth/password/login',{email,password});
  const pending=await start();
  assert.equal((await verify({...pending,code:'00000000'},original)).status,401);
  assert.equal((await verify(pending,'short')).status,400);
  assert.equal((await f.request('/v1/auth/email/verify',pending)).status,401);
  const first=await verify(pending,original);
  assert.equal(first.status,200);
  assert.equal(first.value.account.id,legacy.account.id);
  assert.equal((await f.request('/v1/account',null,legacy.token)).status,401);
  assert.equal((await verify(pending,replacement)).status,401);
  const key=f.db.prepare('SELECT * FROM passwords').get();
  assert.equal(key.salt.length,16);assert.equal(key.digest.length,32);
  assert.equal((await login('wrong password')).status,401);
  const second=await login(original);assert.equal(second.value.account.id,legacy.account.id);
  assert.deepEqual((await f.request('/v1/sync',null,second.value.token)).value,{revision:1,envelope});
  const old=await f.request('/v1/auth/email/start',{email});
  assert.equal((await f.request('/v1/auth/email/verify',{challenge:old.value.challenge,code:f.emails.at(-1).code})).status,403);
  const recovered=await verify(await start(),replacement);assert.equal(recovered.value.account.id,legacy.account.id);
  assert.equal((await f.request('/v1/account',null,second.value.token)).status,401);
  assert.equal((await login(original)).status,401);assert.equal((await login(replacement)).status,200);
 }finally{await f.close()}
});
test('a fresh password account needs a verified email and receives its own salt',async()=>{
 const f=await fixture(),password='an exclusively synthetic passphrase';
 try{
  for(const email of ['new@example.com','another@example.com']){
   const start=await f.request('/v1/auth/password/start',{email});
   assert.equal((await f.request('/v1/auth/password/login',{email,password})).status,401);
   const result=await f.request('/v1/auth/password/verify',{challenge:start.value.challenge,code:f.emails.at(-1).code,password});
   assert.equal(result.status,200);assert.equal(result.value.account.name,email);
  }
  const keys=f.db.prepare('SELECT * FROM passwords').all();
  assert.notDeepEqual(keys[0].salt,keys[1].salt);assert.notDeepEqual(keys[0].digest,keys[1].digest);
 }finally{await f.close()}
});
test('Discord uses the registered callback and requires the desktop proof and one-use loopback ticket',async()=>{
 const f=await fixture({}, {discordId:'123456789012345678',discordSecret:'synthetic-secret'});
 const proof='A'.repeat(64),localState='B'.repeat(64),port=49523;
 try{
  assert.equal((await f.request('/v1/config')).value.discord,true);
  const start=async()=>{const r=await f.request('/v1/auth/discord/start',{proof,localState,port});assert.equal(r.status,200);return new URL(r.value.authorize);};
  const auth=await start();
  assert.equal(auth.origin,'https://discord.com');assert.equal(auth.searchParams.get('scope'),'identify');
  assert.equal(auth.searchParams.get('redirect_uri'),'http://127.0.0.1/v1/auth/discord/callback');
  assert.ok(!auth.href.includes(proof)&&!auth.href.includes(localState));
  const callback=(state,query='code=synthetic')=>f.request('/v1/auth/discord/callback?state='+state+'&'+query);
  const state=auth.searchParams.get('state');
  assert.equal((await callback('X'.repeat(43))).status,401);
  const delivered=await callback(state);assert.equal(delivered.status,303);
  const destination=new URL(delivered.response.headers.get('location'));
  assert.equal(destination.origin,'http://127.0.0.1:'+port);assert.equal(destination.pathname,'/elix/discord');assert.equal(destination.searchParams.get('state'),localState);
  const ticket=destination.searchParams.get('ticket');
  assert.equal((await callback(state)).status,401);
  const complete=(ticket,proof)=>f.request('/v1/auth/discord/complete',{ticket,proof});
  assert.equal((await complete(ticket,'C'.repeat(64))).status,401);
  const login=await complete(ticket,proof);assert.equal(login.status,200);assert.equal(login.value.account.provider,'discord');
  assert.equal((await complete(ticket,proof)).status,401);
  const cancelled=await callback((await start()).searchParams.get('state'),'error=access_denied');
  assert.equal(new URL(cancelled.response.headers.get('location')).searchParams.get('error'),'cancelled');
  const expired=(await start()).searchParams.get('state');f.db.exec('UPDATE challenges SET expires=0');
  assert.equal((await callback(expired)).status,401);assert.equal(f.calls,1);
 }finally{await f.close()}
});
