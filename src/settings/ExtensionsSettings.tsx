import {useCallback,useEffect,useRef,useState} from 'react';
import {FolderOpen,FileArchive,ExternalLink,Puzzle,RefreshCw,Trash2} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';

interface Extension {id:string;name:string;enabled:boolean}

export default function ExtensionsSettings({profile,onError}:{profile:string;onError:(error:unknown)=>void}) {
  const [items,setItems]=useState<Extension[]>([]);
  const [loading,setLoading]=useState(false);
  const [busy,setBusy]=useState(false);
  const [failure,setFailure]=useState('');
  const [remove,setRemove]=useState<string|null>(null);
  const [search,setSearch]=useState('');
  const profileRef=useRef(profile),request=useRef(0);profileRef.current=profile;
  const refresh=useCallback(async()=>{
    if(!desktop)return;
    const sequence=++request.current;
    setLoading(true);setFailure('');
    try{const items=await rpc<Extension[]>('extensions');if(profileRef.current===profile&&request.current===sequence)setItems(items);}
    catch(error){if(profileRef.current===profile&&request.current===sequence)setFailure(error instanceof Error?error.message:String(error));}
    finally{if(profileRef.current===profile&&request.current===sequence)setLoading(false);}
  },[profile]);
  useEffect(()=>{setItems([]);setRemove(null);void refresh();},[refresh]);
  async function act(action:string,data:Record<string,unknown>={}){
    setBusy(true);
    try{await rpc(action,data);setRemove(null);await refresh();}catch(error){onError(error);}finally{setBusy(false);}
  }
  return <div className="settings-block">
    <h2><Puzzle size={22}/>Utvidelser</h2>
    <p>Finn utvidelser i Chrome Nettmarked, og installer kompatible kildefiler som utgiveren tilbyr som ZIP eller mappe. Utvidelser er av i private faner.</p>
    <form className="inline-actions" onSubmit={event=>{event.preventDefault();void act('new_tab',{url:search.trim()?`https://chromewebstore.google.com/search/${encodeURIComponent(search.trim())}`:'https://chromewebstore.google.com/category/extensions'});}}>
      <input aria-label="Søk i Chrome Nettmarked" placeholder="Søk etter en utvidelse …" value={search} maxLength={160} onChange={event=>setSearch(event.target.value)}/>
      <button disabled={!desktop||busy}><ExternalLink size={16}/>Åpne Chrome Nettmarked</button>
    </form>
    <p className="notice">«Legg til i Chrome» installerer ikke i ELIX. Bruk ZIP med utvidelsens kildefiler fra utgiveren, eller en utpakket mappe med manifest.json. Du får se tillatelsene før installering.</p>
    <div className="setting-row"><span>Utvidelser i aktiv profil</span><div className="inline-actions">
      <button disabled={!desktop||busy||loading} onClick={()=>void refresh()}><RefreshCw size={16}/>Oppdater liste</button>
      <button disabled={!desktop||busy} onClick={()=>void act('add_extension',{package:true})}><FileArchive size={16}/>Importer ZIP fra utgiver</button>
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
    <p className="muted">ZIP-kildefiler lagres sammen med profilen. En mappe du velger selv må bli liggende på samme sted og ikke endres mens utvidelsen brukes. Installer bare fra utgivere du stoler på; utvidelser kan lese og endre nettsider.</p>
    <p className="muted">ELIX kontrollerer ikke Chrome Nettmarked-signaturer og importerer ikke CRX-filer. Butikkoppdateringer, full Chrome-kompatibilitet og utvidelsenes egne verktøylinjeknapper/popup-vinduer er ikke tilgjengelige. Nettmarkedet åpnes for å finne utvidelser og utgiverens informasjon.</p>
  </div>;
}
