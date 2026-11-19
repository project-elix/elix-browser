import {useEffect,useState} from 'react';
import {rpc} from '../browser/bridge';

interface SavedSession { name:string; revision:number; updated:string }

export default function SavedSessions({profile,onError}:{profile:string;onError:(error:unknown)=>void}) {
 const [sessions,setSessions]=useState<SavedSession[]>([]);
 const [name,setName]=useState('');
 const [status,setStatus]=useState('');
 const [busy,setBusy]=useState(false);
 useEffect(()=>{
  let current=true;
  rpc<SavedSession[]>('saved_sessions',{profile}).then(rows=>{if(current)setSessions(rows)})
   .catch(error=>{if(current)onError(error)});
  return ()=>{current=false};
 },[profile,onError]);
 async function save() {
  if(busy||!name.trim())return;
  setBusy(true);setStatus('');
  try {
   const trimmed=name.trim();
   await rpc('save_session',{profile,name:trimmed});
   setSessions(await rpc<SavedSession[]>('saved_sessions',{profile}));
   setName('');setStatus(`Lagret «${trimmed}».`);
  } catch(error){onError(error)} finally {setBusy(false)}
 }
 async function open(saved:SavedSession) {
  setBusy(true);setStatus('');
  try {
   await rpc('open_session',{profile,name:saved.name});
   setStatus(`Åpnet «${saved.name}» i et nytt vindu.`);
  } catch(error){onError(error)} finally {setBusy(false)}
 }
 return <div className="gear-section-content" id="gear-sessions">
  <form className="rail-group-form" onSubmit={e=>{e.preventDefault();void save()}}>
   <label className="rail-field">Øktnavn<input value={name} maxLength={80} onChange={e=>setName(e.target.value)}/></label>
   <button className="subtle" disabled={busy||!name.trim()}>Lagre økt</button>
  </form>
  <small>Private faner utelates. Lagrede økter åpnes i et nytt vindu.</small>
  {sessions.map(saved=><button type="button" className="rail-tool" key={saved.name} disabled={busy}
    aria-label={`Åpne økten ${saved.name}`} onClick={()=>void open(saved)}>
    <span>{saved.name} · v{saved.revision}</span></button>)}
  {status&&<small role="status">{status}</small>}
 </div>;
}
