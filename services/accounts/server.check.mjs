import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createAccounts} from './server.mjs';
const fixture=async()=>{
 const emails=[];let calls=0;
 const app=createAccounts({publicUrl:'http://127.0.0.1',development:true,database:':memory:',pepper:'test-only-pepper-not-for-production-123',discordId:'123',discordSecret:'test-secret'},{sendEmail:async(email,code)=>emails.push({email,code}),discordIdentity:async()=>{calls++;return {id:'123456789012345678',name:'Discord tester'}}});
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
 assert.equal((await f.request('/v1/sync',null,a.token,'DELETE')).status,200);
 assert.equal((await f.request('/v1/sync',null,a.token)).value.revision,0);
 }finally{await f.close()}
});
test('Discord binds state to browser cookie, consumes callback and completion once',async()=>{
 const f=await fixture();try{
 const start=await f.request('/v1/auth/discord/start',{});assert.equal(start.status,200);
 const url=new URL(start.value.authorize_url);const authorization=await f.request(url.pathname+url.search);
 const target=new URL(authorization.response.headers.get('location'));const cookie=authorization.response.headers.get('set-cookie').split(';')[0];
 const callback='/v1/auth/discord/callback?'+new URLSearchParams({state:target.searchParams.get('state'),code:'test-code',iss:'https://discord.com'});
 assert.equal((await f.request(callback)).status,401);assert.equal(f.calls,0);
 assert.equal((await f.request(callback,null,null,'GET',{cookie})).status,200);assert.equal(f.calls,1);
 assert.equal((await f.request(callback,null,null,'GET',{cookie})).status,401);
 const complete=await f.request('/v1/auth/discord/complete',{challenge:start.value.challenge});assert.equal(complete.status,200);assert.equal(complete.value.account.provider,'discord');
 assert.equal((await f.request('/v1/auth/discord/complete',{challenge:start.value.challenge})).status,401);
 }finally{await f.close()}
});
test('expired email challenges and foreign browser origins are rejected',async()=>{
 const f=await fixture();try{
 const start=await f.request('/v1/auth/email/start',{email:'a@example.com'});f.db.exec('UPDATE challenges SET expires=0');
 assert.equal((await f.request('/v1/auth/email/verify',{challenge:start.value.challenge,code:f.emails.at(-1).code})).status,401);
 assert.equal((await f.request('/v1/auth/email/start',{email:'b@example.com'},null,'POST',{origin:'https://evil.example'})).status,403);
 }finally{await f.close()}
});
