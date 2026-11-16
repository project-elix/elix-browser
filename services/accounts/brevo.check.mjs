import {test} from 'node:test';
import assert from 'node:assert/strict';
import {BrevoError, checkBrevo, sendLoginEmail} from './brevo.mjs';
import {readConfig, validateOrigin} from './config.mjs';

const config = {brevoKey:'test-api-key',senderEmail:'login@example.com',senderName:'ELIX Browser'};
const response = (value, status = 200) => new Response(JSON.stringify(value), {status,headers:{'content-type':'application/json'}});

test('Brevo delivery uses one recipient, HTML and text, an expiring request and no redirects', async () => {
  let calls = 0;
  await sendLoginEmail(config, 'person@example.com', '12345678', async (url, init) => {
    calls++;
    assert.equal(url, 'https://api.brevo.com/v3/smtp/email');
    assert.equal(init.method, 'POST');
    assert.equal(init.headers['api-key'], config.brevoKey);
    assert.equal(init.redirect, 'error');
    assert.ok(init.signal instanceof AbortSignal);
    const message = JSON.parse(init.body);
    assert.deepEqual(message.to, [{email:'person@example.com'}]);
    assert.equal(message.sender.email, config.senderEmail);
    assert.match(message.htmlContent, /12345678/);
    assert.match(message.textContent, /12345678/);
    assert.match(message.textContent, /10 minutter/);
    assert.equal(message.headers, undefined); // live login must never silently drop mail
    return response({messageId:'<accepted@example.com>'}, 201);
  });
  assert.equal(calls, 1);
});

test('provider failures and timeouts expose no secrets and never retry a possible delivery', async () => {
  for (const status of [400,401,403,429,500]) {
    let calls = 0;
    await assert.rejects(sendLoginEmail(config, 'person@example.com', '12345678', async () => {
      calls++;
      return response({message:`secret ${config.brevoKey} 12345678 person@example.com`}, status);
    }), error => error instanceof BrevoError && error.status === 503 && !/test-api-key|12345678|person@example/.test(error.message));
    assert.equal(calls, 1);
  }
  await assert.rejects(sendLoginEmail(config, 'person@example.com', '12345678', async () => {throw new Error('secret network details')}), {reason:'network-or-timeout'});
  await assert.rejects(sendLoginEmail(config, 'person@example.com', '12345678', async () => response({})), {reason:'missing-message-id'});
});

test('preflight checks the active sender without sending, and sandbox explicitly drops its dummy message', async () => {
  const calls = [];
  const mockFetch = async (url, init) => {
    calls.push({url,init});
    return url.endsWith('/senders') ? response({senders:[{email:'LOGIN@example.com',active:true}]}) : response({messageId:'<sandbox@example.com>'},201);
  };
  assert.deepEqual(await checkBrevo(config,{fetchImpl:mockFetch}), {sender:'active',sandbox:false,deliveryVerified:false});
  assert.equal(calls.length,1);
  assert.equal(calls[0].init.method,'GET');
  await checkBrevo(config,{fetchImpl:mockFetch,sandbox:true});
  assert.equal(calls.length,3);
  const message = JSON.parse(calls[2].init.body);
  assert.deepEqual(message.headers,{'X-Sib-Sandbox':'drop'});
  assert.deepEqual(message.to,[{email:config.senderEmail}]);
  assert.match(message.textContent,/00000000/);
  await assert.rejects(checkBrevo(config,{fetchImpl:async()=>response({senders:[]})}),{reason:'sender-not-found'});
  await assert.rejects(checkBrevo(config,{fetchImpl:async()=>response({senders:'invalid'})}),{reason:'invalid-provider-response'});
  await assert.rejects(checkBrevo(config,{fetchImpl:async()=>response({senders:[{email:config.senderEmail,active:false}]})}),{reason:'sender-not-active'});
});

test('unconfigured production, placeholder pepper and unsafe service origins fail before any send', async () => {
  const env = {PUBLIC_URL:'https://accounts.example.com',AUTH_PEPPER:'x'.repeat(64),BREVO_API_KEY:config.brevoKey,BREVO_SENDER_EMAIL:config.senderEmail};
  assert.equal(readConfig(env).port,3210);
  assert.throws(()=>readConfig({...env,BREVO_API_KEY:''}),{reason:'missing-api-key'});
  assert.throws(()=>readConfig({...env,BREVO_SENDER_EMAIL:'bad\r\naddress'}),{reason:'invalid-sender'});
  assert.throws(()=>readConfig({...env,AUTH_PEPPER:'REPLACE_WITH_AT_LEAST_32_RANDOM_CHARACTERS'}),/AUTH_PEPPER/);
  assert.throws(()=>readConfig({...env,DATABASE_PATH:':memory:'}),/persistent/);
  assert.throws(()=>readConfig({...env,PORT:'0'}),/PORT/);
  assert.throws(()=>readConfig({...env,TRUST_PROXY:'yes'}),/TRUST_PROXY/);
  for (const origin of ['http://accounts.example.com','https://user:pass@example.com','https://accounts.example.com/path','https://accounts.example.com?key=secret','https://accounts.example.com#x']) assert.throws(()=>validateOrigin(origin),/PUBLIC_URL/);
  assert.throws(()=>validateOrigin('ftp://127.0.0.1',true),/PUBLIC_URL/);
  assert.equal(validateOrigin('http://127.0.0.1',true).protocol,'http:');
  await assert.rejects(sendLoginEmail({...config,senderEmail:''},'person@example.com','12345678',async()=>{assert.fail('must not send')}),{reason:'invalid-sender'});
});
