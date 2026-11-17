import {test} from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import {BrevoError, loginMessage} from './brevo.mjs';
import {readConfig} from './config.mjs';
import {mailConfigured} from './mail.mjs';
import {createAccounts} from './server.mjs';
import {checkSmtp, sendSmtpLoginEmail} from './smtp.mjs';

const config = {mailTransport:'smtp',smtpHost:'smtp-relay.brevo.com',smtpPort:587,smtpUser:'test-user',smtpPassword:'test-only-secret',senderEmail:'login@example.com',senderName:'ELIX Browser'};
const recipient = 'person@example.com';
const code = '12345678';
const sensitive = /test-only-secret|test-user|12345678|person@example/;

test('SMTP requires verified TLS and authentication, sends the shared template once and closes resources', async () => {
  for (const port of [587,465]) {
    let calls = 0, closed = 0;
    await sendSmtpLoginEmail({...config,smtpPort:port}, recipient, code, options => {
      assert.equal(options.host, config.smtpHost);
      assert.equal(options.port, port);
      assert.equal(options.secure, port === 465);
      assert.equal(options.requireTLS, true);
      assert.equal(options.ignoreTLS, false);
      assert.equal(options.opportunisticTLS, false);
      assert.deepEqual(options.tls,{rejectUnauthorized:true,minVersion:'TLSv1.2',servername:config.smtpHost});
      assert.equal(options.forceAuth, true);
      assert.deepEqual(options.auth,{user:config.smtpUser,pass:config.smtpPassword});
      for (const key of ['logger','debug','transactionLog','pool']) assert.equal(options[key],false);
      for (const key of ['disableFileAccess','disableUrlAccess']) assert.equal(options[key],true);
      return {close:()=>closed++,sendMail:async message=>{
        calls++;
        assert.deepEqual(message.envelope,{from:{address:config.senderEmail},to:[{address:recipient}]});
        assert.deepEqual(message.to,[{address:recipient}]);
        assert.deepEqual(message.from,{name:config.senderName,address:config.senderEmail});
        const template = loginMessage(config,recipient,code);
        assert.equal(message.text,template.textContent);
        assert.equal(message.html,template.htmlContent);
        assert.equal(message.subject,template.subject);
        assert.ok(message.signal instanceof AbortSignal);
        return {accepted:[recipient],rejected:[]};
      }};
    });
    assert.equal(calls,1);
    assert.equal(closed,1);
  }
});

test('SMTP configuration needs no API key and invalid credentials fail before connecting', async () => {
  const env = {PUBLIC_URL:'https://accounts.example.com',AUTH_PEPPER:'x'.repeat(64),MAIL_TRANSPORT:'smtp',SMTP_HOST:config.smtpHost,SMTP_USER:config.smtpUser,SMTP_PASSWORD:config.smtpPassword,BREVO_SENDER_EMAIL:config.senderEmail};
  const loaded = readConfig(env);
  assert.equal(loaded.smtpPort,587);
  assert.equal(loaded.brevoKey,undefined);
  assert.equal(mailConfigured(loaded),true);
  assert.equal(mailConfigured({...loaded,smtpPassword:undefined}),false);
  for (const changes of [{SMTP_USER:''},{SMTP_PASSWORD:''},{SMTP_PASSWORD:'password\r\nAUTH bad'},{SMTP_HOST:'smtp://host'},{SMTP_PORT:'0'},{SMTP_PORT:'587.5'},{SMTP_PORT:'65536'},{BREVO_SENDER_EMAIL:'sender@example.com,other@example.com'},{MAIL_TRANSPORT:'automatic'}]) assert.throws(()=>readConfig({...env,...changes}));
  for (const email of ['a@example.com,b@example.com','a@example.com\0','a;test@example.com','a\r\nb@example.com','a[b]@example.com','.a@example.com','a..b@example.com']) {
    await assert.rejects(sendSmtpLoginEmail(config,email,code,()=>assert.fail('must not connect')),{reason:'invalid-message'});
  }
  await assert.rejects(sendSmtpLoginEmail({...config,smtpPassword:''},recipient,code,()=>assert.fail('must not connect')),{reason:'missing-smtp-credentials'});
});

test('Nodemailer IDN domain normalization preserves an accepted recipient and its localpart', async () => {
  const nodemailer = (await import('nodemailer')).default;
  const transport = nodemailer.createTransport({jsonTransport:true});
  await sendSmtpLoginEmail(config,'Person+tag@blå.no',code,()=>({
    async sendMail(message) {
      const result = await transport.sendMail(message);
      assert.deepEqual(result.envelope.to,['Person+tag@xn--bl-zia.no']);
      return {accepted:result.envelope.to,rejected:[]};
    },
    close:()=>transport.close(),
  }));
  await assert.rejects(sendSmtpLoginEmail(config,'Person@blå.no',code,()=>({
    sendMail:async()=>({accepted:['Other@xn--bl-zia.no'],rejected:[]}),close(){},
  })),{reason:'smtp-recipient-rejected'});
});

test('SMTP rejection, authentication, network and cleanup errors never expose details or retry', async () => {
  for (const [providerCode,reason] of [['EAUTH','smtp-auth-rejected'],['ECONNECTION','smtp-unavailable'],['EABORT','smtp-timeout']]) {
    let calls = 0, closed = 0;
    await assert.rejects(sendSmtpLoginEmail(config,recipient,code,()=>({
      sendMail:async()=>{calls++;throw Object.assign(new Error(`${config.smtpPassword} ${config.smtpUser} ${recipient} ${code}`),{code:providerCode});},
      close:()=>{closed++;throw new Error(config.smtpPassword);},
    })),error=>error instanceof BrevoError && error.status===503 && error.reason===reason && !sensitive.test(error.stack));
    assert.equal(calls,1);
    assert.equal(closed,1);
  }
  for (const result of [{accepted:[],rejected:[recipient]},{accepted:['other@example.com']},{accepted:[recipient],rejected:[recipient]},{}]) {
    await assert.rejects(sendSmtpLoginEmail(config,recipient,code,()=>({close(){},sendMail:async()=>result})),{reason:'smtp-recipient-rejected'});
  }
});

test('SMTP preflight verifies TLS and authentication only and does not send mail', async () => {
  let verifies = 0, closed = 0;
  const result = await checkSmtp(config,{createTransport:()=>({verify:async()=>{verifies++;return true;},sendMail:()=>assert.fail('preflight must not send'),close:()=>closed++})});
  assert.deepEqual(result,{transport:'smtp',authenticated:true,senderVerified:false,deliveryVerified:false});
  assert.equal(verifies,1);
  assert.equal(closed,1);
  await assert.rejects(checkSmtp(config,{createTransport:()=>({verify:async()=>{throw Object.assign(new Error(config.smtpPassword),{code:'EAUTH'});},close(){}})}),{reason:'smtp-auth-rejected'});
});

async function mockSmtp(t,{silent=false}={}) {
  const commands = [], sockets = new Set();
  const server = net.createServer(socket=>{
    sockets.add(socket);
    socket.on('close',()=>sockets.delete(socket));
    socket.on('error',()=>{});
    if (silent) return;
    socket.write('220 localhost test SMTP\r\n');
    let buffer = '';
    socket.on('data',data=>{
      buffer += data.toString();
      while (buffer.includes('\r\n')) {
        const end = buffer.indexOf('\r\n'), line = buffer.slice(0,end);
        buffer = buffer.slice(end+2);
        commands.push(line);
        if (/^EHLO /.test(line)) socket.write('250-localhost\r\n250 AUTH PLAIN LOGIN\r\n');
        else if (line === 'STARTTLS') socket.write('502 TLS is unavailable\r\n');
        else socket.write('500 Unexpected command\r\n');
      }
    });
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  t.after(async()=>{for(const socket of sockets) socket.destroy();await new Promise(resolve=>server.close(resolve));});
  return {commands,sockets,config:{...config,smtpHost:'127.0.0.1',smtpPort:server.address().port}};
}

test('real Nodemailer refuses an SMTP server without STARTTLS before credentials or mail leave the process', async t => {
  const mock = await mockSmtp(t);
  await assert.rejects(sendSmtpLoginEmail(mock.config,recipient,code),{reason:'smtp-unavailable'});
  await assert.rejects(checkSmtp(mock.config),{reason:'smtp-unavailable'});
  assert.equal(mock.commands.filter(line=>line==='STARTTLS').length,2);
  assert.equal(mock.commands.some(line=>/^(AUTH|MAIL|RCPT|DATA)\b/.test(line)),false);
});

test('send deadline aborts the actual Nodemailer socket instead of leaving delivery running', async t => {
  const mock = await mockSmtp(t,{silent:true});
  const timeout = AbortSignal.timeout.bind(AbortSignal);
  t.mock.method(AbortSignal,'timeout',milliseconds=>{
    assert.equal(milliseconds,18000);
    return timeout(100);
  });
  await assert.rejects(sendSmtpLoginEmail(mock.config,recipient,code),{reason:'smtp-timeout'});
  // Closing is observable at the mock peer on the next socket event.
  await Promise.all([...mock.sockets].map(socket=>new Promise(resolve=>socket.once('close',resolve))));
  assert.equal(mock.sockets.size,0);
});

test('account route uses SMTP, advertises its configuration and removes failed login challenges', async t => {
  const mock = await mockSmtp(t);
  const app = createAccounts({...mock.config,publicUrl:'http://127.0.0.1',development:true,database:':memory:',pepper:'test-only-pepper-with-at-least-32-characters'});
  await new Promise(resolve=>app.server.listen(0,'127.0.0.1',resolve));
  t.after(()=>app.close());
  const origin = `http://127.0.0.1:${app.server.address().port}`;
  const status = await (await fetch(origin+'/v1/config')).json();
  assert.equal(status.email,true);
  const response = await fetch(origin+'/v1/auth/email/start',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({email:recipient})});
  assert.equal(response.status,503);
  assert.equal(sensitive.test(await response.text()),false);
  for (const table of ['challenges','users','sessions']) assert.equal(app.db.prepare(`SELECT count(*) AS n FROM ${table}`).get().n,0);
  assert.equal(mock.commands.some(line=>/^AUTH /.test(line)),false);
});
