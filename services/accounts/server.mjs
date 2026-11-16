import {createServer} from 'node:http';
import {DatabaseSync} from 'node:sqlite';
import {randomBytes,randomInt,createHash,createHmac,timingSafeEqual} from 'node:crypto';
import {mkdirSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const digest=s=>createHash('sha256').update(s).digest('hex');
const random=()=>randomBytes(32).toString('base64url');
const seconds=()=>Math.floor(Date.now()/1000);
const equal=(a,b)=>typeof a==='string'&&typeof b==='string'&&a.length===b.length&&timingSafeEqual(Buffer.from(a),Buffer.from(b));
const fault=(status,message)=>Object.assign(new Error(message),{status});
const emailOf=v=>{if(typeof v!=='string'||v.length>254||!/^\S+@[^\s@]+\.[^\s@]+$/.test(v))throw fault(400,'Skriv en gyldig e-postadresse');return v.trim().toLowerCase();};
function envelope(v){
  if(!v||v.version!==1||v.algorithm!=='AES-GCM'||v.iterations!==600000||typeof v.salt!=='string'||typeof v.iv!=='string'||typeof v.ciphertext!=='string')throw fault(400,'Ugyldig kryptert innhold');
  for(const [key,length,max] of [['salt',16,16],['iv',12,12],['ciphertext',16,4_000_000]]){
    if(!/^[A-Za-z0-9+/]+={0,2}$/.test(v[key]))throw fault(400,'Ugyldig kryptert innhold');
    const n=Buffer.from(v[key],'base64').length;if(n<length||n>max)throw fault(400,'Ugyldig kryptert innhold');
  }
  return Object.fromEntries(['version','algorithm','iterations','salt','iv','ciphertext'].map(k=>[k,v[k]]));
}

export function createAccounts(config,providers={}){
  const base=new URL(config.publicUrl);
  if(base.protocol!=='https:'&&!(config.development&&base.hostname==='127.0.0.1'))throw new Error('PUBLIC_URL requires HTTPS');
  if(!config.pepper||config.pepper.length<32)throw new Error('AUTH_PEPPER must contain at least 32 random characters');
  const hash=s=>createHmac('sha256',config.pepper).update(s).digest('hex');
  if(config.database!==':memory:')mkdirSync(dirname(resolve(config.database)),{recursive:true,mode:0o700});
  const db=new DatabaseSync(config.database);
  db.exec(`PRAGMA journal_mode=WAL;PRAGMA foreign_keys=ON;PRAGMA busy_timeout=5000;
    CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY, provider TEXT NOT NULL, subject TEXT NOT NULL, name TEXT NOT NULL, UNIQUE(provider,subject));
    CREATE TABLE IF NOT EXISTS sessions(token TEXT PRIMARY KEY, user_id TEXT REFERENCES users(id), expires INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS challenges(id TEXT PRIMARY KEY, provider TEXT NOT NULL, subject TEXT, code TEXT, expires INTEGER NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, state TEXT UNIQUE, binding TEXT, user_id TEXT, status TEXT NOT NULL DEFAULT 'pending');
    CREATE TABLE IF NOT EXISTS limits(key TEXT PRIMARY KEY, count INTEGER NOT NULL, expires INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS sync(user_id TEXT PRIMARY KEY REFERENCES users(id), revision INTEGER NOT NULL, envelope TEXT NOT NULL);`);
  const q=sql=>db.prepare(sql);
  const limit=(key,max,window=3600)=>{const k=hash(key+':'+Math.floor(seconds()/window));q('INSERT INTO limits VALUES(?,1,?) ON CONFLICT(key) DO UPDATE SET count=count+1').run(k,seconds()+window);if(q('SELECT count FROM limits WHERE key=?').get(k).count>max)throw fault(429,'For mange forsøk. Prøv igjen senere.');};
  const json=(res,status,v)=>{res.writeHead(status,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store','X-Content-Type-Options':'nosniff'});res.end(JSON.stringify(v));};
  const body=async req=>{if(!req.headers['content-type']?.startsWith('application/json'))throw fault(415,'JSON kreves');let n=0;const chunks=[];for await(const chunk of req){n+=chunk.length;if(n>5_500_000)throw fault(413,'For stor forespørsel');chunks.push(chunk);}try{return JSON.parse(Buffer.concat(chunks));}catch{throw fault(400,'Ugyldig JSON');}};
  const user=(provider,subject,name)=>{let u=q('SELECT * FROM users WHERE provider=? AND subject=?').get(provider,subject);if(!u){const id=random();q('INSERT INTO users VALUES(?,?,?,?)').run(id,provider,subject,String(name).slice(0,80));u=q('SELECT * FROM users WHERE id=?').get(id);}return u;};
  const publicUser=u=>({id:u.id,provider:u.provider,name:u.name});
  const issue=u=>{const token=random();q('INSERT INTO sessions VALUES(?,?,?)').run(digest(token),u.id,seconds()+30*86400);return {token,account:publicUser(u)};};
  const authenticate=req=>{const token=req.headers.authorization?.replace(/^Bearer /,'');if(!token||token.length>100)throw fault(401,'Logg inn på nytt');const u=q('SELECT users.* FROM sessions JOIN users ON users.id=sessions.user_id WHERE token=? AND expires>?').get(digest(token),seconds());if(!u)throw fault(401,'Logg inn på nytt');return u;};
  const sendEmail=providers.sendEmail??(async(email,code)=>{
    if(!config.brevoKey||!config.senderEmail)throw fault(503,'E-postinnlogging er ikke konfigurert');
    const r=await fetch('https://api.brevo.com/v3/smtp/email',{method:'POST',headers:{'api-key':config.brevoKey,'Content-Type':'application/json'},body:JSON.stringify({sender:{email:config.senderEmail,name:'ELIX Browser'},to:[{email}],subject:'Din innloggingskode til ELIX',textContent:`Din innloggingskode er ${code}. Koden er gyldig i 10 minutter. Ikke del den med andre. Hvis du ikke ba om koden, kan du se bort fra e-posten.`}),signal:AbortSignal.timeout(15000)});
    if(!r.ok)throw fault(503,'Kunne ikke sende e-post akkurat nå');
  });
  const discordIdentity=providers.discordIdentity??(async code=>{
    const tokenResponse=await fetch('https://discord.com/api/oauth2/token',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded'},body:new URLSearchParams({client_id:config.discordId,client_secret:config.discordSecret,grant_type:'authorization_code',code,redirect_uri:new URL('/v1/auth/discord/callback',base).href}),signal:AbortSignal.timeout(15000)});
    if(!tokenResponse.ok)throw fault(401,'Discord-innlogging mislyktes');
    const token=await tokenResponse.json();
    const identity=await fetch('https://discord.com/api/v10/users/@me',{headers:{Authorization:`Bearer ${token.access_token}`},signal:AbortSignal.timeout(15000)});
    if(!identity.ok)throw fault(401,'Kunne ikke bekrefte Discord-kontoen');
    const value=await identity.json();
    if(!/^\d{15,22}$/.test(value.id))throw fault(401,'Ugyldig Discord-konto');
    return {id:value.id,name:value.global_name||value.username};
  });
  let cleaned=0;
  const server=createServer(async(req,res)=>{
    try{
      if(seconds()-cleaned>60){for(const table of ['limits','sessions','challenges'])q(`DELETE FROM ${table} WHERE expires<?`).run(seconds());cleaned=seconds();}
      const url=new URL(req.url,base);const path=url.pathname;
      const ip=config.trustProxy?String(req.headers['x-forwarded-for']??req.socket.remoteAddress).split(',')[0].trim():req.socket.remoteAddress;
      if(req.headers.origin&&req.headers.origin!==base.origin)throw fault(403,'Ugyldig opprinnelse');
      limit('all:'+ip,1200,600);
      if(path==='/health'&&req.method==='GET')return json(res,200,{ok:true});
      if(path==='/v1/config'&&req.method==='GET')return json(res,200,{email:!!(providers.sendEmail||(config.brevoKey&&config.senderEmail)),discord:!!(config.discordId&&config.discordSecret),sync:true});
      if(path==='/v1/auth/email/start'&&req.method==='POST'){
        const b=await body(req),email=emailOf(b.email);limit('email:'+email,5);limit('email-ip:'+ip,20);
        const id=random(),code=String(randomInt(10000000,100000000));
        q('INSERT INTO challenges(id,provider,subject,code,expires) VALUES(?,?,?,?,?)').run(digest(id),'email',email,hash(id+':'+code),seconds()+600);
        try{await sendEmail(email,code);}catch(e){q('DELETE FROM challenges WHERE id=?').run(digest(id));throw e;}
        return json(res,200,{challenge:id,expires_in:600});
      }
      if(path==='/v1/auth/email/verify'&&req.method==='POST'){
        limit('verify:'+ip,100,600);const b=await body(req);
        if(typeof b.challenge!=='string'||b.challenge.length>100||typeof b.code!=='string'||!/^\d{8}$/.test(b.code))throw fault(400,'Skriv den åttesifrede koden');
        const c=q('SELECT * FROM challenges WHERE id=? AND provider=? AND expires>?').get(digest(b.challenge),'email',seconds());
        if(!c||c.attempts>=5||c.status!=='pending')throw fault(401,'Koden er ugyldig eller utløpt');
        q('UPDATE challenges SET attempts=attempts+1 WHERE id=?').run(c.id);
        if(!equal(c.code,hash(b.challenge+':'+b.code)))throw fault(401,'Koden er ugyldig eller utløpt');
        q('DELETE FROM challenges WHERE id=?').run(c.id);
        return json(res,200,issue(user('email',c.subject,c.subject)));
      }
      if(path==='/v1/auth/discord/start'&&req.method==='POST'){
        await body(req);limit('discord:'+ip,20);
        if(!config.discordId||!config.discordSecret)throw fault(503,'Discord er ikke konfigurert');
        const id=random();q('INSERT INTO challenges(id,provider,expires) VALUES(?,?,?)').run(digest(id),'discord',seconds()+600);
        return json(res,200,{challenge:id,authorize_url:new URL('/v1/auth/discord/authorize?request='+id,base).href});
      }
      const cookieName=base.protocol==='https:'?'__Host-elix_oauth':'elix_oauth_dev';
      const cookieValue=req.headers.cookie?.split(';').map(x=>x.trim()).find(x=>x.startsWith(cookieName+'='))?.slice(cookieName.length+1);
      if(path==='/v1/auth/discord/authorize'&&req.method==='GET'){
        const id=url.searchParams.get('request')??'';const c=q('SELECT * FROM challenges WHERE id=? AND provider=? AND expires>?').get(digest(id),'discord',seconds());
        if(!c||c.status!=='pending')throw fault(400,'Innloggingen er utløpt. Start på nytt i ELIX.');
        const state=random(),binding=random();q("UPDATE challenges SET state=?,binding=?,status='authorizing' WHERE id=?").run(hash(state),hash(binding),c.id);
        const target=new URL('https://discord.com/oauth2/authorize');target.search=new URLSearchParams({client_id:config.discordId,response_type:'code',scope:'identify',state,redirect_uri:new URL('/v1/auth/discord/callback',base).href,prompt:'consent'}).toString();
        res.writeHead(302,{Location:target.href,'Set-Cookie':`${cookieName}=${binding}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600${base.protocol==='https:'?'; Secure':''}`,'Cache-Control':'no-store','Referrer-Policy':'no-referrer'});return res.end();
      }
      if(path==='/v1/auth/discord/callback'&&req.method==='GET'){
        const state=url.searchParams.get('state')??'';const c=q('SELECT * FROM challenges WHERE state=? AND provider=? AND expires>?').get(hash(state),'discord',seconds());
        if(!c||c.status!=='authorizing'||!cookieValue||!equal(c.binding,hash(cookieValue))||url.searchParams.get('iss')!=='https://discord.com')throw fault(401,'Discord-forespørselen kunne ikke bekreftes');
        q("UPDATE challenges SET status='processing',state=NULL,binding=NULL WHERE id=?").run(c.id);
        const code=url.searchParams.get('code');if(!code||code.length>512)throw fault(401,'Innloggingen ble avbrutt');
        const identity=await discordIdentity(code);const u=user('discord',identity.id,identity.name);
        q("UPDATE challenges SET user_id=?,status='ready' WHERE id=?").run(u.id,c.id);
        res.writeHead(200,{'Content-Type':'text/html; charset=utf-8','Cache-Control':'no-store','Content-Security-Policy':"default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'",'Referrer-Policy':'no-referrer','Set-Cookie':`${cookieName}=; Max-Age=0; HttpOnly; SameSite=Lax; Path=/${base.protocol==='https:'?'; Secure':''}`});
        return res.end('<!doctype html><html lang="nb"><meta charset="utf-8"><title>ELIX – innlogget</title><body style="background:#080b12;color:#edf5ff;font:18px system-ui;padding:8vw"><h1>Du er klar.</h1><p>Gå tilbake til ELIX-profilen og trykk «Fullfør Discord-innlogging».</p></body></html>');
      }
      if(path==='/v1/auth/discord/complete'&&req.method==='POST'){
        limit('poll:'+ip,300,600);const b=await body(req);if(typeof b.challenge!=='string'||b.challenge.length>100)throw fault(400,'Ugyldig forespørsel');
        const c=q('SELECT * FROM challenges WHERE id=? AND provider=? AND expires>?').get(digest(b.challenge),'discord',seconds());
        if(!c)throw fault(401,'Innloggingen er utløpt');if(c.status!=='ready')return json(res,202,{pending:true});
        q('DELETE FROM challenges WHERE id=?').run(c.id);return json(res,200,issue(q('SELECT * FROM users WHERE id=?').get(c.user_id)));
      }
      const u=authenticate(req);
      if(path==='/v1/account'&&req.method==='GET')return json(res,200,{account:publicUser(u)});
      if(path==='/v1/auth/logout'&&req.method==='POST'){await body(req);q('DELETE FROM sessions WHERE token=?').run(digest(req.headers.authorization.slice(7)));return json(res,200,{ok:true});}
      if(path==='/v1/sync'&&req.method==='GET'){const row=q('SELECT * FROM sync WHERE user_id=?').get(u.id);return json(res,200,row?{revision:row.revision,envelope:JSON.parse(row.envelope)}:{revision:0,envelope:null});}
      if(path==='/v1/sync'&&req.method==='PUT'){
        limit('sync:'+u.id,60,600);const b=await body(req);if(!Number.isSafeInteger(b.revision)||b.revision<0)throw fault(400,'Ugyldig revisjon');const encrypted=JSON.stringify(envelope(b.envelope));
        let changed;
        if(b.revision===0){changed=q('INSERT OR IGNORE INTO sync VALUES(?,1,?)').run(u.id,encrypted).changes;}else{changed=q('UPDATE sync SET revision=revision+1,envelope=? WHERE user_id=? AND revision=?').run(encrypted,u.id,b.revision).changes;}
        if(!changed)throw fault(409,'En annen enhet har nyere data. Hent dem før du laster opp.');
        return json(res,200,{revision:b.revision+1});
      }
      if(path==='/v1/sync'&&req.method==='DELETE'){q('DELETE FROM sync WHERE user_id=?').run(u.id);return json(res,200,{ok:true});}
      throw fault(404,'Fant ikke endepunktet');
    }catch(error){if(!res.headersSent)json(res,error.status??500,{error:error.status?error.message:'Tjenesten kunne ikke fullføre forespørselen'});else res.end();}
  });
  server.requestTimeout=20000;server.headersTimeout=15000;
  return {server,db,close:async()=>{await new Promise(r=>server.close(r));db.close();}};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
  const {server}=createAccounts({publicUrl:process.env.PUBLIC_URL,pepper:process.env.AUTH_PEPPER,database:process.env.DATABASE_PATH??'./data/accounts.sqlite',brevoKey:process.env.BREVO_API_KEY,senderEmail:process.env.BREVO_SENDER_EMAIL,discordId:process.env.DISCORD_CLIENT_ID,discordSecret:process.env.DISCORD_CLIENT_SECRET,trustProxy:process.env.TRUST_PROXY==='1',development:process.env.NODE_ENV==='development'});
  server.listen(Number(process.env.PORT??3210),'127.0.0.1',()=>console.log('ELIX accounts ready'));
}
