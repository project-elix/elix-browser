import {useState} from 'react';
import {KeyRound,Trash2} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
import type {Settings} from '../types';

export default function PasswordSettings({settings,onSave,onError}:{settings:Settings;onSave:(settings:Settings)=>Promise<void>;onError:(error:unknown)=>void}){
  const [busy,setBusy]=useState(false);
  const passwordAutosave=settings.password_autosave,autofill=settings.autofill;
  function onChange(patch:Partial<Settings>){setBusy(true);void onSave({...settings,...patch}).catch(onError).finally(()=>setBusy(false));}
  const [confirm,setConfirm]=useState(false),[clearing,setClearing]=useState(false),[cleared,setCleared]=useState(false);
  async function clear(){setClearing(true);setCleared(false);try{await rpc('clear_passwords',{consent:true});setConfirm(false);setCleared(true);}catch(error){onError(error);}finally{setClearing(false);}}
  return <div className="settings-block">
    <h2><KeyRound size={22}/>Passord og autofyll</h2>
    <label className="setting-row"><span><strong>Tilby å lagre passord</strong><small style={{display:'block'}}>Viser nettleserens Lagre / Oppdater-spørsmål når du logger inn.</small></span><input type="checkbox" checked={passwordAutosave} disabled={busy||!desktop} onChange={event=>onChange({password_autosave:event.target.checked})}/></label>
    <label className="setting-row"><span><strong>Autofyll navn og adresser</strong><small style={{display:'block'}}>Husker skjemafelter i denne nettleserprofilen.</small></span><input type="checkbox" checked={autofill} disabled={busy||!desktop} onChange={event=>onChange({autofill:event.target.checked})}/></label>
    <p>Passord lagres og fylles inn av WebView2 i den aktive profilen. De tas ikke med i ELIX-loggen eller diagnostikkeksporten. Private faner lagrer ikke passord.</p>
    <p className="notice">Når passordlagring slås av, kan tidligere lagrede passord fortsatt fylles inn. Slett dem nedenfor hvis du også vil fjerne eksisterende opplysninger. Visning og redigering av enkeltpassord er foreløpig ikke tilgjengelig.</p>
    <button disabled={busy||clearing||!desktop} onClick={()=>{setConfirm(true);setCleared(false);}}><Trash2 size={16}/>Slett lagrede passord i denne profilen</button>
    {confirm&&<div className="notice" role="alert"><p>Dette sletter alle passord lagret av ELIX i den aktive profilen. Handlingen kan ikke angres.</p><div className="inline-actions"><button disabled={clearing} onClick={()=>void clear()}>{clearing?'Sletter …':'Ja, slett passordene'}</button><button disabled={clearing} onClick={()=>setConfirm(false)}>Avbryt</button></div></div>}
    {cleared&&<p role="status">Lagrede passord er slettet i denne profilen.</p>}
  </div>;
}
