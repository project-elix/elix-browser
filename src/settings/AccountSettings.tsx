import {useCallback,useEffect,useRef,useState} from 'react';
import {Cloud,Mail,ShieldCheck} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
import {encrypt,decrypt,type Envelope,type SyncData} from '../profiles/sync';
import {expiredSessionMessage,readPendingLogin,startPendingLogin,type PendingEmailLogin} from '../profiles/login';
interface Account{id:string;provider:string;name:string}
interface Providers{password?:boolean;email:boolean;discord:boolean}
interface Status{configured:boolean;account:Account|null}
const cancelled=new Error('Account view closed');

export default function AccountSettings({profile,onRefresh,onError}:{profile:string;onRefresh:()=>Promise<void>;onError:(e:unknown)=>void}){
 const pendingKey=`elix-login:${profile}`;
 const [pending,setPending]=useState<PendingEmailLogin|null>(()=>{try{return readPendingLogin(sessionStorage.getItem(pendingKey))}catch{return null}});
 const [status,setStatus]=useState<Status|null>(null),[email,setEmail]=useState(pending?.email??''),[code,setCode]=useState('');
 const [busy,setBusy]=useState(false),[message,setMessage]=useState(''),[statusFailed,setStatusFailed]=useState(false);
 const [providers,setProviders]=useState<Providers|null>(null),[providersFailed,setProvidersFailed]=useState(false);
 const [password,setPassword]=useState(''),[setup,setSetup]=useState(false),[editing,setEditing]=useState(false),[discordBusy,setDiscordBusy]=useState(false);
 const discordPending=useRef(false);
 const [enabled,setEnabled]=useState(false),[phrase,setPhrase]=useState(''),[confirmDelete,setConfirmDelete]=useState<{revision:number}|null>(null);
 const active=useRef(true),running=useRef(false),codeInput=useRef<HTMLInputElement>(null);
 useEffect(()=>{active.current=true;return()=>{active.current=false;if(discordPending.current)void rpc('account',{profile,operation:'discord_cancel',payload:{}}).catch(()=>{})}},[profile]);
 const call=async<T,>(operation:string,payload:unknown={}):Promise<T>=>{
  const result=await rpc<T>('account',{profile,operation,payload});
  // A response belonging to a profile/settings tab that was left must not alter the new view.
  if(!active.current)throw cancelled;
  return result;
 };
 const remember=useCallback((value:PendingEmailLogin|null)=>{
  setPending(value);
  try{if(value)sessionStorage.setItem(pendingKey,JSON.stringify(value));else sessionStorage.removeItem(pendingKey)}catch{/* Login also works when session storage is unavailable. */}
 },[pendingKey]);
 useEffect(()=>{
  let current=true;
  if(!desktop){setStatus({configured:false,account:null});return}
  void rpc<Status>('account',{profile,operation:'status',payload:{}}).then(v=>{if(current)setStatus(v)}).catch(e=>{if(current){setStatusFailed(true);onError(e)}});
  return()=>{current=false};
 },[profile,onError]);
 useEffect(()=>{
  if(!desktop||!status?.configured)return;
  let current=true;
  void rpc<Providers>('account',{profile,operation:'providers',payload:{}}).then(v=>{if(current){setProviders(v);setProvidersFailed(false)}})
    .catch(()=>{if(current)setProvidersFailed(true)});
  return()=>{current=false};
 },[profile,status?.configured]);
 useEffect(()=>{
  if(!pending)return;
  const timeout=window.setTimeout(()=>{remember(null);setCode('');setMessage('Koden er utløpt. Send en ny kode for å logge inn.');},Math.max(0,pending.expiresAt-Date.now()));
  return()=>window.clearTimeout(timeout);
 },[pending,remember]);
 const run=async(fn:()=>Promise<void>)=>{
  if(running.current)return;
  running.current=true;setBusy(true);setMessage('');
  try{await fn()}catch(e){
   if(active.current&&e!==cancelled){
    if(String(e)===expiredSessionMessage||e instanceof Error&&e.message===expiredSessionMessage){
     setStatus({configured:true,account:null});setEnabled(false);setPhrase('');setConfirmDelete(null);remember(null);
    }
    onError(e);
   }
  }finally{running.current=false;if(active.current)setBusy(false)}
 };
 const sendCode=async()=>{
  const address=email.trim().toLowerCase();
  const kind=pending?.kind??(providers?.password?'password':'email');
  const result=await call<{challenge:string;expires_in:number}>(kind==='password'?'password_start':'email_start',{email:address});
  remember(startPendingLogin(result.challenge,address,result.expires_in,Date.now(),kind));setEmail(address);setCode('');
  setMessage(`Koden er sendt til ${address} og er gyldig i 10 minutter.`);
  requestAnimationFrame(()=>{if(active.current)codeInput.current?.focus()});
 };
 const complete=async()=>{
  if(!pending||pending.expiresAt<=Date.now()){remember(null);setCode('');throw new Error('Koden er utløpt. Send en ny kode.');}
  const result=await call<Status>(pending.kind==='password'?'password_verify':'email_verify',{challenge:pending.challenge,code,...(pending.kind==='password'?{password}:{})});
  setPassword('');setSetup(false);setEditing(false);
  setStatus(result);remember(null);setCode('');setMessage('Du er innlogget. Synkronisering er valgfri.');
 };
 const revisionKey=`elix-sync-revision:${status?.account?.id??''}:${profile}`;
 return <div className="account-settings">
  <h3><ShieldCheck size={18}/>ELIX-konto</h3>
  <p>Du kan bruke ELIX med eller uten konto.</p>
  {!status?<div>
   <p role="status">{statusFailed?'Kontostatus kunne ikke hentes.':'Henter kontostatus …'}</p>
   {statusFailed&&<button className="subtle" disabled={busy} onClick={()=>void run(async()=>{setStatus(await call<Status>('status'));setStatusFailed(false);})}>Prøv igjen</button>}
  </div>:!status.configured?<div className="unavailable">
   <strong>Kontoinnlogging er ikke aktivert i dette bygget.</strong>
   <p>Lokale profiler fungerer. E-post og nettbasert synkronisering åpnes når kontotjenesten er satt opp.</p>
  </div>:status.account&&!editing?<>
   <div className="setting-row"><div><strong>{status.account.name}</strong><p>Innlogget med {status.account.provider==='discord'?'Discord':'e-post'}</p></div>
    <button className="subtle" disabled={busy} onClick={()=>void run(async()=>{await call('logout');setStatus({configured:true,account:null});setEnabled(false);setPhrase('');setConfirmDelete(null);remember(null);setMessage('Du er logget ut. Lokale bokmerker og faner er beholdt.');})}>Logg ut</button>
   </div>
   {providers?.email&&providers.password&&status.account.provider==='email'&&<button className="subtle" disabled={busy} onClick={()=>{setEmail(status.account!.name);setEditing(true);setSetup(true);remember(null);setPassword('');setMessage('');}}>Opprett eller endre passord</button>}
   <h3><Cloud size={18}/>Kryptert synkronisering</h3>
   <label className="setting-row"><div><strong>Bruk kryptert synkronisering</strong><p>Bokmerker og utseende. Du velger selv når du laster opp eller henter.</p></div><input type="checkbox" role="switch" checked={enabled} disabled={busy} onChange={e=>{setEnabled(e.target.checked);if(!e.target.checked){setPhrase('');setConfirmDelete(null)}}}/></label>
   {enabled&&<>
    <label className="phrase-field">Synkroniseringsfrase<input type="password" autoComplete="off" minLength={12} value={phrase} disabled={busy} onChange={e=>setPhrase(e.target.value)} placeholder="Minst 12 tegn – samme frase på alle enheter"/></label>
    <p className="muted">Frasen sendes aldri til serveren og kan ikke tilbakestilles. Hent fra den andre enheten før du laster opp for å slå sammen bokmerkene. Henting erstatter utseendeinnstillingene dine.</p>
    <div className="account-actions">
     <button className="subtle" disabled={busy||phrase.length<12} onClick={()=>void run(async()=>{
      const data=await rpc<SyncData>('sync_export',{profile});if(!active.current)throw cancelled;
      const encrypted=await encrypt(data,phrase);if(!active.current)throw cancelled;
      const result=await call<{revision:number}>('sync_put',{revision:Number(sessionStorage.getItem(revisionKey)||0),envelope:encrypted});
      sessionStorage.setItem(revisionKey,String(result.revision));setPhrase('');setMessage('Krypterte bokmerker og utseende er lastet opp.');
     })}>Last opp kryptert</button>
     <button className="subtle" disabled={busy||phrase.length<12} onClick={()=>void run(async()=>{
      const result=await call<{revision:number;envelope:Envelope|null}>('sync_get');
      if(!result.envelope){sessionStorage.setItem(revisionKey,String(result.revision));setMessage('Ingen synkroniserte data ennå.');return;}
      const data=await decrypt(result.envelope,phrase);if(!active.current)throw cancelled;
      await rpc('sync_apply',{profile,items:data.items,groups:data.groups??[],appearance:data.appearance});if(!active.current)throw cancelled;
      sessionStorage.setItem(revisionKey,String(result.revision));setPhrase('');await onRefresh();if(active.current)setMessage('Bokmerkene er slått sammen og utseendet er hentet.');
     })}>Hent og slå sammen</button>
     <button className="subtle" disabled={busy} onClick={()=>void run(async()=>{const result=await call<{revision:number}>('sync_get');setConfirmDelete({revision:result.revision});})}>Slett fra serveren</button>
    </div>
    {confirmDelete&&<div className="notice">Slette synkroniseringskopien? Lokale bokmerker beholdes.
     <button disabled={busy} onClick={()=>void run(async()=>{const result=await call<{revision:number}>('sync_delete',{revision:confirmDelete.revision});sessionStorage.setItem(revisionKey,String(result.revision));setConfirmDelete(null);setMessage('Synkroniseringskopien er slettet.');})}>Slett</button>
     <button disabled={busy} onClick={()=>setConfirmDelete(null)}>Avbryt</button>
    </div>}
   </>}
  </>:<>
   {pending?<>
    <p>En åttesifret kode er sendt til <strong>{pending.email}</strong>.</p>
    <form className="inline-form" onSubmit={e=>{e.preventDefault();void run(complete)}}>
     <input ref={codeInput} inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{8}" maxLength={8} required aria-label="Bekreftelseskode" placeholder="Åttesifret kode" value={code} disabled={busy} onChange={e=>setCode(e.target.value.replace(/\D/g,''))}/>
     {pending.kind==='password'&&<input type="password" autoComplete="new-password" required aria-label="Nytt passord, minst 15 tegn" placeholder="Nytt passord (minst 15 tegn)" value={password} disabled={busy} onChange={e=>setPassword(e.target.value)}/>}
     <button className="primary" disabled={busy||code.length!==8||(pending.kind==='password'&&[...password].length<15)}>{pending.kind==='password'?'Lagre passord og logg inn':'Logg inn'}</button>
    </form>
    <div className="account-actions">
     <button className="subtle" disabled={busy} onClick={()=>void run(sendCode)}>Send ny kode</button>
     <button className="subtle" disabled={busy} onClick={()=>{remember(null);setCode('');setPassword('');setMessage('');}}>Tilbake</button>
    </div>
   </>:!providers?<div>{providersFailed?<button className="subtle" disabled={busy} onClick={()=>void run(async()=>setProviders(await call<Providers>('providers')))}>Hent innloggingsmetoder på nytt</button>:<p role="status">Henter innloggingsmetoder …</p>}</div>:<>
    {(providers.email||providers.password)&&<>
     {setup&&<p>Vi sender en kode til e-posten din før du velger nytt passord. Bruk samme adresse som før for å beholde kontoen og synkroniseringen.</p>}
     <form className="inline-form" onSubmit={e=>{e.preventDefault();void run(async()=>{
        if(setup||!providers.password){await sendCode();return;}
        const result=await call<Status>('password_login',{email:email.trim(),password});
        setStatus(result);setPassword('');setMessage('Du er innlogget.');
     })}}>
      <input type="email" required aria-label="E-postadresse" autoComplete="username" placeholder="Din e-postadresse" value={email} disabled={busy||editing} onChange={e=>setEmail(e.target.value)}/>
      {providers.password&&!setup&&<input type="password" required aria-label="Passord" autoComplete="current-password" placeholder="Passord" value={password} disabled={busy} onChange={e=>setPassword(e.target.value)}/>}
      <button className="primary" disabled={busy}>{providers.password&&!setup?'Logg inn':<><Mail size={15}/>Send kode</>}</button>
     </form>
     {providers.password&&providers.email&&<button className="subtle" disabled={busy} onClick={()=>{setEditing(false);setSetup(!setup);setPassword('');setMessage('');}}>{setup?'Tilbake':'Opprett konto eller få nytt passord'}</button>}
    </>}
    {providers.discord&&!editing&&<div className="account-actions">
     <button className="subtle" disabled={busy} onClick={()=>void run(async()=>{
       discordPending.current=true;setDiscordBusy(true);setMessage('Fullfør innloggingen hos Discord i nettleseren.');
       try{const result=await call<Status>('discord_login');setStatus(result);setPassword('');setMessage('Du er innlogget med Discord.');}
       finally{discordPending.current=false;if(active.current)setDiscordBusy(false);}
     })}>Logg inn med Discord</button>
     {discordBusy&&<button className="subtle" onClick={()=>void call('discord_cancel').catch(onError)}>Avbryt</button>}
    </div>}
   </>}
  </>}
  {message&&<p role="status" className="account-message">{message}</p>}
 </div>;
}
