import {useCallback,useEffect,useRef,useState} from 'react';
import {Cloud,Mail,ShieldCheck} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
import {encrypt,decrypt,type Envelope,type SyncData} from '../profiles/sync';
import {expiredSessionMessage,readPendingLogin,startPendingLogin,type PendingEmailLogin} from '../profiles/login';
interface Account{id:string;provider:string;name:string}
interface Status{configured:boolean;account:Account|null}
const cancelled=new Error('Account view closed');

export default function AccountSettings({profile,onRefresh,onError}:{profile:string;onRefresh:()=>Promise<void>;onError:(e:unknown)=>void}){
 const pendingKey=`elix-login:${profile}`;
 const [pending,setPending]=useState<PendingEmailLogin|null>(()=>{try{return readPendingLogin(sessionStorage.getItem(pendingKey))}catch{return null}});
 const [status,setStatus]=useState<Status|null>(null),[email,setEmail]=useState(pending?.email??''),[code,setCode]=useState('');
 const [busy,setBusy]=useState(false),[message,setMessage]=useState(''),[statusFailed,setStatusFailed]=useState(false);
 const [enabled,setEnabled]=useState(false),[phrase,setPhrase]=useState(''),[confirmDelete,setConfirmDelete]=useState<{revision:number}|null>(null);
 const active=useRef(true),running=useRef(false),codeInput=useRef<HTMLInputElement>(null);
 useEffect(()=>{active.current=true;return()=>{active.current=false}},[]);
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
  const result=await call<{challenge:string;expires_in:number}>('email_start',{email:address});
  remember(startPendingLogin(result.challenge,address,result.expires_in));setEmail(address);setCode('');
  setMessage(`Koden er sendt til ${address} og er gyldig i 10 minutter.`);
  requestAnimationFrame(()=>{if(active.current)codeInput.current?.focus()});
 };
 const complete=async()=>{
  if(!pending||pending.expiresAt<=Date.now()){remember(null);setCode('');throw new Error('Koden er utløpt. Send en ny kode.');}
  const result=await call<Status>('email_verify',{challenge:pending.challenge,code});
  setStatus(result);remember(null);setCode('');setMessage('Du er innlogget. Synkronisering er valgfri.');
 };
 const revisionKey=`elix-sync-revision:${status?.account?.id??''}:${profile}`;
 return <div className="account-settings">
  <h3><ShieldCheck size={18}/>ELIX-konto</h3>
  <p>Logg inn med e-post og en engangskode. Du kan fortsatt bruke ELIX uten konto.</p>
  {!status?<div>
   <p role="status">{statusFailed?'Kontostatus kunne ikke hentes.':'Henter kontostatus …'}</p>
   {statusFailed&&<button className="subtle" disabled={busy} onClick={()=>void run(async()=>{setStatus(await call<Status>('status'));setStatusFailed(false);})}>Prøv igjen</button>}
  </div>:!status.configured?<div className="unavailable">
   <strong>Kontoinnlogging er ikke aktivert i dette bygget.</strong>
   <p>Lokale profiler fungerer. E-post og nettbasert synkronisering åpnes når kontotjenesten er satt opp.</p>
  </div>:status.account?<>
   <div className="setting-row"><div><strong>{status.account.name}</strong><p>Innlogget med {status.account.provider==='discord'?'Discord':'e-post'}</p></div>
    <button className="subtle" disabled={busy} onClick={()=>void run(async()=>{await call('logout');setStatus({configured:true,account:null});setEnabled(false);setPhrase('');setConfirmDelete(null);remember(null);setMessage('Du er logget ut. Lokale bokmerker og faner er beholdt.');})}>Logg ut</button>
   </div>
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
      await rpc('sync_apply',{profile,items:data.items,appearance:data.appearance});if(!active.current)throw cancelled;
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
     <input ref={codeInput} inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{8}" maxLength={8} required aria-label="Innloggingskode" placeholder="Åttesifret kode" value={code} disabled={busy} onChange={e=>setCode(e.target.value.replace(/\D/g,''))}/>
     <button className="primary" disabled={busy||code.length!==8}>{busy?'Venter …':'Logg inn'}</button>
    </form>
    <div className="account-actions">
     <button className="subtle" disabled={busy} onClick={()=>void run(sendCode)}>Send ny kode</button>
     <button className="subtle" disabled={busy} onClick={()=>{remember(null);setCode('');setMessage('');}}>Bruk en annen e-post</button>
    </div>
   </>:<form className="inline-form" onSubmit={e=>{e.preventDefault();void run(sendCode)}}>
    <input type="email" required aria-label="E-postadresse" autoComplete="email" placeholder="Din e-postadresse" value={email} disabled={busy} onChange={e=>setEmail(e.target.value)}/>
    <button className="subtle" disabled={busy}><Mail size={15}/>{busy?'Sender …':'Send kode'}</button>
   </form>}
   <p className="muted">Bruk samme e-postadresse på alle enheter. Discord-innlogging er ikke tilgjengelig i denne versjonen.</p>
  </>}
  {message&&<p role="status" className="account-message">{message}</p>}
 </div>;
}
