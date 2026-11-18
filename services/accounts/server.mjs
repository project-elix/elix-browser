import {createServer} from 'node:http';
import {DatabaseSync} from 'node:sqlite';
import {randomBytes,randomInt,createHash,createHmac,timingSafeEqual} from 'node:crypto';
import {mkdirSync,realpathSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {BrevoError,validEmail} from './brevo.mjs';
import {sendMail,mailConfigured} from './mail.mjs';
import {readConfig,validateOrigin} from './config.mjs';
import {passwordStore,validatePassword} from './password.mjs';
import {discordRoutes} from './discord.mjs';
const digest=s=>createHash('sha256').update(s).digest('hex');
const random=()=>randomBytes(32).toString('base64url');
const seconds=()=>Math.floor(Date.now()/1000);
const equal=(a,b)=>typeof a==='string'&&typeof b==='string'&&a.length===b.length&&timingSafeEqual(Buffer.from(a),Buffer.from(b));
const fault=(status,message)=>Object.assign(new Error(message),{status});
const emailOf=v=>{const value=typeof v==='string'?v.trim().toLowerCase():v;if(!validEmail(value))throw fault(400,'Skriv en gyldig e-postadresse');return value;};
function envelope(v){
  if(!v||v.version!==1||v.algorithm!=='AES-GCM'||v.iterations!==600000||typeof v.salt!=='string'||typeof v.iv!=='string'||typeof v.ciphertext!=='string')throw fault(400,'Ugyldig kryptert innhold');
  for(const [key,length,max] of [['salt',16,16],['iv',12,12],['ciphertext',16,4_000_000]]){
    if(!/^[A-Za-z0-9+/]+={0,2}$/.test(v[key]))throw fault(400,'Ugyldig kryptert innhold');
    const n=Buffer.from(v[key],'base64').length;if(n<length||n>max)throw fault(400,'Ugyldig kryptert innhold');
  }
  return Object.fromEntries(['version','algorithm','iterations','salt','iv','ciphertext'].map(k=>[k,v[k]]));
}

export function createAccounts(config,providers={}){
  const base=validateOrigin(config.publicUrl,config.development);
  if(!config.pepper||config.pepper.length<32)throw new Error('AUTH_PEPPER must contain at least 32 random characters');
  const hash=s=>createHmac('sha256',config.pepper).update(s).digest('hex');
  if(config.database!==':memory:')mkdirSync(dirname(resolve(config.database)),{recursive:true,mode:0o700});
  const db=new DatabaseSync(config.database);
  db.exec(`PRAGMA journal_mode=WAL;PRAGMA foreign_keys=ON;PRAGMA busy_timeout=5000;
    CREATE TABLE IF NOT EXISTS users(id TEXT PRIMARY KEY, provider TEXT NOT NULL, subject TEXT NOT NULL, name TEXT NOT NULL, UNIQUE(provider,subject));
    CREATE TABLE IF NOT EXISTS passwords(user_id TEXT PRIMARY KEY REFERENCES users(id), salt BLOB NOT NULL, digest BLOB NOT NULL);
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
  const sendEmail=providers.sendEmail??((email,code,purpose)=>sendMail(config,email,code,purpose));
  const passwords=passwordStore();
  const credential=email=>q("SELECT p.* FROM passwords p JOIN users u ON p.user_id=u.id WHERE u.provider='email' AND u.subject=?").get(email);
  const discord=discordRoutes({config,base,q,body,json,limit,seconds,user,issue,identity:providers.discordIdentity});
  let cleaned=0;
  const server=createServer(async(req,res)=>{
    try{
      if(seconds()-cleaned>60){for(const table of ['limits','sessions','challenges'])q(`DELETE FROM ${table} WHERE expires<?`).run(seconds());cleaned=seconds();}
      const url=new URL(req.url,base);const path=url.pathname;
      const ip=config.trustProxy?String(req.headers['x-forwarded-for']??req.socket.remoteAddress).split(',')[0].trim():req.socket.remoteAddress;
      if(req.headers.origin&&req.headers.origin!==base.origin)throw fault(403,'Ugyldig opprinnelse');
      limit('all:'+ip,1200,600);
      if(path==='/health'&&req.method==='GET')return json(res,200,{ok:true});
      if(path==='/v1/config'&&req.method==='GET')return json(res,200,{email:!!providers.sendEmail||mailConfigured(config),password:true,discord:discord.enabled,sync:true});
      if(path==='/v1/auth/password/login'&&req.method==='POST'){
        const b=await body(req),email=emailOf(b.email);
        limit('password-ip:'+ip,60,600);limit('password-login:'+email,15,600);
        if(typeof b.password!=='string'||Buffer.byteLength(b.password)>512)throw fault(401,'Feil e-postadresse eller passord.');
        const record=credential(email);
        if(!await passwords.matches(b.password,record))throw fault(401,'Feil e-postadresse eller passord.');
        const current=q('SELECT u.* FROM users u JOIN passwords p ON u.id=p.user_id WHERE u.id=? AND p.digest=?').get(record.user_id,record.digest);
        if(!current)throw fault(401,'Passordet ble endret. Prøv på nytt.');
        return json(res,200,issue(current));
      }
      const passwordFlow=path.startsWith('/v1/auth/password/');
      if((path==='/v1/auth/email/start'||path==='/v1/auth/password/start')&&req.method==='POST'){
        const b=await body(req),email=emailOf(b.email);limit('email:'+email,5);limit('email-ip:'+ip,20);
        const id=random(),code=String(randomInt(10000000,100000000));
        q('INSERT INTO challenges(id,provider,subject,code,expires) VALUES(?,?,?,?,?)').run(digest(id),passwordFlow?'password':'email',email,hash(id+':'+code),seconds()+600);
        try{await sendEmail(email,code,passwordFlow?'password':'email');}catch(e){q('DELETE FROM challenges WHERE id=?').run(digest(id));throw e;}
        return json(res,200,{challenge:id,expires_in:600});
      }
      if((path==='/v1/auth/email/verify'||path==='/v1/auth/password/verify')&&req.method==='POST'){
        limit('verify:'+ip,100,600);const b=await body(req);
        if(typeof b.challenge!=='string'||b.challenge.length>100||typeof b.code!=='string'||!/^\d{8}$/.test(b.code))throw fault(400,'Skriv den åttesifrede koden');
        const c=q('SELECT * FROM challenges WHERE id=? AND provider=? AND expires>?').get(digest(b.challenge),passwordFlow?'password':'email',seconds());
        if(!c||c.attempts>=5||c.status!=='pending')throw fault(401,'Koden er ugyldig eller utløpt');
        q('UPDATE challenges SET attempts=attempts+1 WHERE id=?').run(c.id);
        if(!equal(c.code,hash(b.challenge+':'+b.code)))throw fault(401,'Koden er ugyldig eller utløpt');
        if(passwordFlow){
          validatePassword(b.password);
          const key=await passwords.create(b.password);
          db.exec('BEGIN IMMEDIATE');
          try{
            if(!q('DELETE FROM challenges WHERE id=? AND expires>?').run(c.id,seconds()).changes)throw fault(401,'Koden er ugyldig eller utløpt');
            const account=user('email',c.subject,c.subject);
            q('INSERT INTO passwords VALUES(?,?,?) ON CONFLICT(user_id) DO UPDATE SET salt=excluded.salt,digest=excluded.digest').run(account.id,key.salt,key.digest);
            q('DELETE FROM sessions WHERE user_id=?').run(account.id);
            q("DELETE FROM challenges WHERE subject=? AND provider IN ('email','password')").run(c.subject);
            const session=issue(account);
            db.exec('COMMIT');
            return json(res,200,session);
          }catch(error){db.exec('ROLLBACK');throw error;}
        }
        q('DELETE FROM challenges WHERE id=?').run(c.id);
        if(credential(c.subject))throw fault(403,'Bruk passordet ditt i Alpha 10 eller en nyere versjon.');
        return json(res,200,issue(user('email',c.subject,c.subject)));
      }
      if(path.startsWith('/v1/auth/discord/'))return await discord.handle(req,res,url,ip);
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
      if(path==='/v1/sync'&&req.method==='DELETE'){
        const b=await body(req);if(!Number.isSafeInteger(b.revision)||b.revision<0)throw fault(400,'Ugyldig revisjon');
        // Keep a tombstone: deleting the row would reuse old revisions and let stale devices overwrite newer data.
        const changed=b.revision===0?q("INSERT OR IGNORE INTO sync VALUES(?,1,'null')").run(u.id).changes:q("UPDATE sync SET revision=revision+1,envelope='null' WHERE user_id=? AND revision=?").run(u.id,b.revision).changes;
        if(!changed)throw fault(409,'Synkroniseringskopien er endret. Hent på nytt før du prøver igjen.');
        return json(res,200,{ok:true,revision:b.revision+1});
      }
      throw fault(404,'Fant ikke endepunktet');
    }catch(error){if(error instanceof BrevoError)console.error('ELIX mail provider:',error.reason);if(!res.headersSent)json(res,error.status??500,{error:error.status?error.message:'Tjenesten kunne ikke fullføre forespørselen'});else res.end();}
  });
  server.requestTimeout=20000;server.headersTimeout=15000;
  return {server,db,close:async()=>{await new Promise(r=>server.close(r));db.close();}};
}
function isMainModule(){
  if(!process.argv[1])return false;
  // Resolve both paths: production starts through the current release symlink.
  try{return realpathSync(fileURLToPath(import.meta.url))===realpathSync(process.argv[1]);}catch{return false;}
}
if(isMainModule()){
  process.umask(0o077);
  try{
    const config=readConfig(),app=createAccounts(config);
    app.server.listen(config.port,'127.0.0.1',()=>console.log('ELIX accounts ready'));
    let stopping=false;
    const stop=async()=>{if(stopping)return;stopping=true;const deadline=setTimeout(()=>process.exit(1),25000);deadline.unref();await app.close();clearTimeout(deadline);};
    process.once('SIGTERM',stop);process.once('SIGINT',stop);
  }catch(error){console.error(error instanceof BrevoError?`ELIX accounts configuration: ${error.reason}`:error.message);process.exitCode=1;}
}

