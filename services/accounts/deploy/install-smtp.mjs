#!/usr/bin/env node
// First installation only. No credentials are accepted in arguments or printed.
import {parseArgs, parseEnv} from 'node:util';
import {readFile, writeFile, mkdir, lstat, realpath, chmod, symlink, rename, copyFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {randomBytes} from 'node:crypto';
import {dirname, join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import net from 'node:net';

const ROOT='/opt/elix-accounts', ENV='/etc/elix-accounts.env';
const UNIT='/etc/systemd/system/elix-accounts.service';
const SNIPPET='/etc/nginx/snippets/elix-accounts.conf';
const FILES=['server.mjs','password.mjs','discord.mjs','config.mjs','brevo.mjs','smtp.mjs','mail.mjs','check-mail.mjs','check-brevo.mjs','package.json','package-lock.json'];
const PUBLIC_PATHS=['/v1/config','/v1/account','/v1/sync','/v1/auth/email/start','/v1/auth/email/verify','/v1/auth/password/start','/v1/auth/password/verify','/v1/auth/password/login','/v1/auth/discord/start','/v1/auth/discord/callback','/v1/auth/discord/complete','/v1/auth/logout','/elix-accounts-health'];
const fail=message=>{throw Object.assign(new Error(message),{safeMessage:true});};
const run=(file,args,options={})=>execFileSync(file,args,{encoding:'utf8',stdio:['ignore','pipe','pipe'],...options});
const exists=async path=>{try{await lstat(path);return true;}catch(e){if(e.code==='ENOENT')return false;throw e;}};
function identityExists(kind){
  try{run('getent',[kind,'elix-accounts']);return true;}
  catch(e){if(e.status===2)return false;fail('Cannot check the dedicated service identity');}
}
export function assertFirstInstall(state){
  if(!state.portFree||state.environment||state.unit||state.current||state.data||state.user||state.group)fail('First-install-only helper: existing state or identity detected; no overwrite performed');
}
async function safeDirectory(path){
  if(!await exists(path))return;
  const st=await lstat(path);
  if(!st.isDirectory()||st.uid!==0||(st.mode&0o022))fail('Installation parent directories must be real root-owned directories without group/world write');
}
async function routesUnused(origin){
  for(const path of PUBLIC_PATHS){
    const r=await request(origin+path);
    console.log(JSON.stringify({route:path,status:r.status,contentType:r.type}));
    if(r.status!==404)fail('A proposed public route is occupied or cannot be confirmed unused');
  }
}

export function originOf(value){
  const u=new URL(value);
  if(u.protocol!=='https:'||u.username||u.password||u.pathname!=='/'||u.search||u.hash)fail('Use an HTTPS origin without a path');
  return u.origin;
}
export function mappedMail(env){
  const out={MAIL_TRANSPORT:'smtp',SMTP_HOST:env.APP_SMTP_HOST,SMTP_PORT:env.APP_SMTP_PORT||'587',SMTP_USER:env.APP_SMTP_USER,SMTP_PASSWORD:env.APP_SMTP_PASSWORD,BREVO_SENDER_EMAIL:env.APP_MAIL_FROM,BREVO_SENDER_NAME:'ELIX Browser'};
  for(const [name,value] of Object.entries(out))if(typeof value!=='string'||!value.trim()||/[\r\n\0'"]/.test(value))fail(`Missing or unsupported value for ${name}`);
  if(!['587','465'].includes(out.SMTP_PORT))fail('Use SMTP port 587 (STARTTLS) or 465 (TLS)');
  if(!/^[a-z0-9.-]+$/i.test(out.SMTP_HOST)||!out.SMTP_HOST.includes('.'))fail('Invalid SMTP hostname');
  if(!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(out.BREVO_SENDER_EMAIL))fail('Invalid sender address');
  return out;
}
export function envText(env){return Object.entries(env).map(([k,v])=>{
  if(!/^[A-Z_]+$/.test(k)||/[\r\n\0'"]/.test(String(v)))fail('Unsupported environment value');
  return `${k}='${v}'`;
}).join('\n')+'\n';}
async function readMail(paths){
  if(!paths.length)fail('Specify source environment files in their actual override order');
  const merged={};
  for(const path of paths){
    if(!path.startsWith('/'))fail('Source environment paths must be absolute');
    const st=await lstat(path);
    if(!st.isFile()||st.uid!==0||(st.mode&0o027))fail('Source environment files must be root-owned regular files without world access or group write');
    Object.assign(merged,parseEnv(await readFile(path,'utf8')));
  }
  return mappedMail(merged);
}
async function portFree(){return await new Promise(resolve=>{
  const s=net.connect({host:'127.0.0.1',port:3210});
  s.setTimeout(2000);
  s.once('connect',()=>{s.destroy();resolve(false);});
  s.once('error',e=>resolve(e.code==='ECONNREFUSED'));
  s.once('timeout',()=>{s.destroy();resolve(false);});
});}
async function request(url,{timeoutMs=10000}={}){
  const r=await fetch(url,{redirect:'error',headers:{Connection:'close'},signal:AbortSignal.timeout(timeoutMs)});
  return {status:r.status,type:r.headers.get('content-type')||'',body:await r.text()};
}
export async function verifyPublic(origin,{send=request,now=Date.now,sleep=ms=>new Promise(r=>setTimeout(r,ms))}={}){
  const deadline=now()+10000;
  let result={ok:false,healthStatus:null,configStatus:null};
  while(now()<deadline){
    const timeoutMs=Math.max(1,Math.min(2000,deadline-now()));
    const responses=await Promise.allSettled([
      send(origin+'/elix-accounts-health',{timeoutMs}),send(origin+'/v1/config',{timeoutMs}),
    ]);
    const values=responses.map(response=>response.status==='fulfilled'?response.value:null);
    const valid=(response,key)=>{try{return response?.status===200&&JSON.parse(response.body)[key]===true;}catch{return false;}};
    result={ok:valid(values[0],'ok')&&valid(values[1],'email'),healthStatus:values[0]?.status??null,configStatus:values[1]?.status??null};
    if(result.ok)return result;
    const remaining=deadline-now();if(remaining>0)await sleep(Math.min(250,remaining));
  }
  return result;
}
export function assertReusableSnippet(stat,contents){
  if(!stat.isFile()||stat.uid!==0||(stat.mode&0o022)||contents!==nginxSnippet())fail('Existing accounts snippet differs or is unsafe; no overwrite performed');
}
async function checkExistingSnippet(){
  const stat=await lstat(SNIPPET);
  // lstat rejects symlinks even when they point to an otherwise valid snippet.
  if(!stat.isFile())fail('Existing accounts snippet differs or is unsafe; no overwrite performed');
  assertReusableSnippet(stat,await readFile(SNIPPET,'utf8'));
}
export function nginxSnippet(){
  const common=`        client_max_body_size 6m;\n        access_log off;\n        proxy_http_version 1.1;\n        proxy_set_header Host $host;\n        proxy_set_header X-Forwarded-Proto https;\n        proxy_set_header X-Forwarded-For $remote_addr;\n        proxy_set_header Connection "";\n        proxy_connect_timeout 5s;\n        proxy_read_timeout 40s;\n        proxy_send_timeout 40s;\n`;
  return '# ELIX Browser account routes; existing membership routes remain unchanged.\n'+[
    ['= /v1/config',''],['= /v1/account',''],['= /v1/sync',''],['^~ /v1/auth/',''],['= /elix-accounts-health','/health']
  ].map(([location,uri])=>`    location ${location} {\n${common}        proxy_pass http://127.0.0.1:3210${uri};\n    }\n`).join('\n');
}
export function addNginxInclude(original,origin){
  const marker='    client_max_body_size 64k;';
  if(original.split(marker).length!==2||original.includes('elix-accounts.conf')||/location\s+[^\n]*\/v1(?:\/|\b)/.test(original))fail('Nginx layout differs from the reviewed layout; inspect manually');
  const pos=original.indexOf(marker), start=original.lastIndexOf('server {',pos), header=original.slice(start,pos);
  if(start<0||!/listen\s+443\b[^;]*\bssl\b/.test(header)||!header.includes('server_name '+new URL(origin).hostname+';'))fail('The insertion point is not in the matching HTTPS server block');
  return original.replace(marker,marker+'\n    include '+SNIPPET+';');
}
export function serviceText(node){
  if(!/^\/[A-Za-z0-9/_.-]+$/.test(node))fail('Unsupported Node executable path');
  return `[Unit]\nDescription=ELIX Browser account service\nWants=network-online.target\nAfter=network-online.target\n\n[Service]\nType=simple\nUser=elix-accounts\nGroup=elix-accounts\nWorkingDirectory=${ROOT}/current\nEnvironmentFile=${ENV}\nEnvironment=NODE_ENV=production\nExecStart=${node} ${ROOT}/current/server.mjs\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=30\nStateDirectory=elix-accounts\nStateDirectoryMode=0700\nUMask=0077\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectControlGroups=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\nCapabilityBoundingSet=\n\n[Install]\nWantedBy=multi-user.target\n`;
}
async function preflight(opts){
  if(process.getuid?.()!==0)fail('Run as root to inspect protected configuration');
  if(Number(process.versions.node.split('.')[0])!==24)fail('Run this helper using Node 24');
  const origin=originOf(opts.origin),mail=await readMail(opts['smtp-env']||[]);
  for(const path of ['/opt',ROOT,ROOT+'/releases'])await safeDirectory(path);
  const free=await portFree();
  console.log(JSON.stringify({node:process.versions.node,origin,smtpHost:mail.SMTP_HOST,smtpPort:mail.SMTP_PORT,sender:mail.BREVO_SENDER_EMAIL,credentialsPresent:true,port3210Free:free,environmentExists:await exists(ENV),unitExists:await exists(UNIT),currentExists:await exists(ROOT+'/current')}));
  if(opts['nginx-file']){
    const file=await realpath(opts['nginx-file']);
    const original=await readFile(file,'utf8');
    addNginxInclude(original,origin);
    console.log('Reviewed HTTPS insertion point found; no Nginx changes made');
  }
  await routesUnused(origin);
  return {origin,mail,free};
}
async function install(opts){
  const {origin,mail,free}=await preflight(opts);
  assertFirstInstall({portFree:free,environment:await exists(ENV),unit:await exists(UNIT),current:await exists(ROOT+'/current'),data:await exists('/var/lib/elix-accounts'),user:identityExists('passwd'),group:identityExists('group')});
  if(!/^[a-f0-9]{40}$/.test(opts.revision||''))fail('Supply the reviewed full 40-character Git commit');
  const node=await realpath(process.execPath), npm=join(dirname(node),'npm');
  if(!await exists(npm))fail('npm must exist beside the selected Node executable');
  const release=ROOT+'/releases/'+opts.revision;
  if(await exists(release))fail('Release directory exists; inspect the prior attempt before retrying');
  await mkdir(release,{recursive:true,mode:0o755});
  for(const file of FILES){
    const r=await fetch(`https://raw.githubusercontent.com/project-elix/elix-browser/${opts.revision}/services/accounts/${file}`,{redirect:'error',signal:AbortSignal.timeout(20000)});
    if(!r.ok)fail('Failed to fetch a pinned runtime file');
    const body=await r.text();if(body.length>2_000_000)fail('Runtime file exceeds size limit');
    await writeFile(join(release,file),body,{mode:0o644,flag:'wx'});
  }
  const lock=JSON.parse(await readFile(join(release,'package-lock.json'),'utf8'));
  for(const item of Object.values(lock.packages||{}))if(item.resolved&&(!item.resolved.startsWith('https://registry.npmjs.org/')||!item.integrity))fail('Runtime lockfile contains an unpinned or unexpected package source');
  const npmGlobal=join(release,'npm-global.conf');
  await writeFile(npmGlobal,'',{mode:0o600,flag:'wx'});
  run(node,[npm,'ci','--omit=dev','--ignore-scripts','--no-audit','--no-fund','--registry=https://registry.npmjs.org/','--userconfig=/dev/null','--globalconfig='+npmGlobal],{cwd:release,env:{PATH:dirname(node)+':/usr/bin:/bin',HOME:'/root',npm_config_cache:'/root/.npm'}});
  const config={...mail,NODE_ENV:'production',PUBLIC_URL:origin,AUTH_PEPPER:randomBytes(48).toString('base64url'),DATABASE_PATH:'/var/lib/elix-accounts/accounts.sqlite',PORT:'3210',TRUST_PROXY:'1'};
  await writeFile(ENV,envText(config),{mode:0o600,flag:'wx'});
  // verify() authenticates over verified TLS; it sends no message.
  run(node,['--env-file='+ENV,join(release,'check-mail.mjs')],{env:{PATH:dirname(node)+':/usr/bin:/bin'}});
  run('useradd',['--system','--user-group','--home','/var/lib/elix-accounts','--shell','/usr/sbin/nologin','elix-accounts']);
  await symlink(release,ROOT+'/current');
  await writeFile(UNIT,serviceText(node),{mode:0o644,flag:'wx'});
  run('systemctl',['daemon-reload']);
  try{
    run('systemctl',['enable','--now','elix-accounts.service']);
    let healthy=false;
    for(let attempt=0;attempt<15;attempt++){
      try{const r=await request('http://127.0.0.1:3210/health');if(r.status===200&&JSON.parse(r.body).ok){healthy=true;break;}}catch{}
      await new Promise(r=>setTimeout(r,1000));
    }
    if(!healthy)fail('Loopback health check failed');
    console.log('SMTP TLS/authentication and loopback health verified. Service installed; Nginx unchanged. No email sent.');
  }catch{
    try{run('systemctl',['disable','--now','elix-accounts.service']);}catch{}
    fail('New account service failed startup and was disabled. Files retained for inspection; existing services unchanged');
  }
}
async function publish(opts){
  if(process.getuid?.()!==0)fail('Run as root');
  const origin=originOf(opts.origin);
  const config=parseEnv(await readFile(ENV,'utf8'));
  if(config.PUBLIC_URL!==origin)fail('Origin differs from the installed service');
  const health=await request('http://127.0.0.1:3210/health');
  if(health.status!==200||!JSON.parse(health.body).ok)fail('Loopback service is not healthy');
  if(!opts['nginx-file'])fail('Specify the exact reviewed Nginx virtual-host file');
  const file=await realpath(opts['nginx-file']),original=await readFile(file,'utf8');
  const st=await lstat(file);if(!st.isFile()||st.uid!==0||(st.mode&0o022))fail('Nginx virtual-host file must be root-owned without group/world write');
  const replacement=addNginxInclude(original,origin);
  const snippetExists=await exists(SNIPPET);
  if(snippetExists)await checkExistingSnippet();
  await routesUnused(origin);
  const backup='/root/elix-accounts-nginx-'+Date.now()+'.conf';
  await writeFile(backup,original,{mode:0o600,flag:'wx'});
  await mkdir(dirname(SNIPPET),{recursive:true,mode:0o755});
  if(snippetExists)await checkExistingSnippet();
  else await writeFile(SNIPPET,nginxSnippet(),{mode:0o644,flag:'wx'});
  const tmp=file+'.elix-accounts.tmp';
  await writeFile(tmp,replacement,{mode:0o644,flag:'wx'});
  if(await readFile(file,'utf8')!==original)fail('Nginx file changed during preparation; no replacement performed');
  await rename(tmp,file);
  let phase='syntax check',publicResult;
  try{
    run('nginx',['-t']);phase='reload';run('systemctl',['reload','nginx']);
    phase='public verification';publicResult=await verifyPublic(origin);
    if(!publicResult.ok)fail('Public verification failed');
    console.log('HTTPS health/config verified. Existing membership proxy preserved. Backup: '+backup);
  }catch{
    const diagnosis='Nginx '+phase+' failed'+(publicResult?` (health HTTP ${publicResult.healthStatus??'unavailable'}, config HTTP ${publicResult.configStatus??'unavailable'})`:'');
    if(await readFile(file,'utf8')!==replacement)fail(diagnosis+'; Nginx file changed concurrently; automatic restore withheld. The protected backup is retained for review');
    await copyFile(backup,file);await chmod(file,0o644);
    try{run('nginx',['-t']);run('systemctl',['reload','nginx']);}catch{fail(diagnosis+' and rollback reload failed. Inspect Nginx immediately; original file restored');}
    fail(diagnosis+'; original virtual-host file restored and reloaded');
  }
}
export async function main(args){
  const {values,positionals}=parseArgs({args,allowPositionals:true,options:{origin:{type:'string',short:'o'},revision:{type:'string',short:'r'},'smtp-env':{type:'string',multiple:true,short:'e'},'nginx-file':{type:'string',short:'n'}}});
  if(positionals.length!==1)fail('Use preflight, install or publish');
  if(positionals[0]==='preflight')await preflight(values);
  else if(positionals[0]==='install')await install(values);
  else if(positionals[0]==='publish')await publish(values);
  else fail('Use preflight, install or publish');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  main(process.argv.slice(2)).catch(e=>{console.error(e?.safeMessage?e.message:'Deployment operation failed; private/provider output withheld');process.exitCode=1;});
}
