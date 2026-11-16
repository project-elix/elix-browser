import {useCallback,useEffect,useState} from 'react';
import {FolderOpen,Puzzle,RefreshCw,Trash2} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';

interface Extension {id:string;name:string;enabled:boolean}

export default function ExtensionsSettings({profile,onError}:{profile:string;onError:(error:unknown)=>void}) {
  const [items,setItems]=useState<Extension[]>([]);
  const [loading,setLoading]=useState(false);
  const [busy,setBusy]=useState(false);
  const [failure,setFailure]=useState('');
  const [remove,setRemove]=useState<string|null>(null);
  const refresh=useCallback(async()=>{
    if(!desktop)return;
    setLoading(true);setFailure('');
    try{setItems(await rpc<Extension[]>('extensions'));}
    catch(error){setFailure(error instanceof Error?error.message:String(error));}
    finally{setLoading(false);}
  },[profile]);
  useEffect(()=>{setItems([]);void refresh();},[refresh]);
  async function act(action:string,data:Record<string,unknown>={}){
    setBusy(true);
    try{await rpc(action,data);setRemove(null);await refresh();}catch(error){onError(error);}finally{setBusy(false);}
  }
  return <div className="settings-block">
    <h2><Puzzle size={22}/>Utvidelser</h2>
    <p>Installer utpakkede utvidelser for denne profilen. Utvidelser er av i private faner.</p>
    <p className="notice">Velg en mappe med manifest.json fra en kilde du stoler på. Utvidelser kan lese og endre nettsider. Behold mappen på samme sted etter installering.</p>
    <div className="setting-row"><span>Utvidelser i aktiv profil</span><div className="inline-actions">
      <button disabled={!desktop||busy||loading} onClick={()=>void refresh()}><RefreshCw size={16}/>Oppdater liste</button>
      <button disabled={!desktop||busy} onClick={()=>void act('add_extension')}><FolderOpen size={16}/>Velg utvidelsesmappe</button>
    </div></div>
    {loading&&<p role="status">Henter utvidelser …</p>}
    {failure&&<p role="alert" className="notice">{failure}</p>}
    {!loading&&!failure&&!items.length&&<p>Ingen utvidelser installert i denne profilen.</p>}
    {items.map(item=><div className="settings-block" key={item.id}>
      <div className="setting-row"><span><strong>{item.name}</strong><small style={{display:'block',overflowWrap:'anywhere'}}>{item.id}</small></span>
        <div className="inline-actions"><label><input type="checkbox" checked={item.enabled} disabled={busy} onChange={event=>void act('enable_extension',{id:item.id,enabled:event.target.checked})}/>Aktiv</label>
        <button disabled={busy} onClick={()=>setRemove(item.id)} aria-label={`Fjern ${item.name}`}><Trash2 size={16}/>Fjern</button></div>
      </div>
      {remove===item.id&&<div className="notice"><p>Fjerne {item.name} fra denne profilen?</p><div className="inline-actions"><button disabled={busy} onClick={()=>void act('remove_extension',{id:item.id})}>Fjern utvidelse</button><button disabled={busy} onClick={()=>setRemove(null)}>Avbryt</button></div></div>}
    </div>)}
    <p className="muted">Installasjon direkte fra Chrome Nettmarked er ikke støttet. Utvidelser som krever en egen verktøylinjeknapp eller popup, kan ha begrenset funksjonalitet i ELIX.</p>
  </div>;
}
