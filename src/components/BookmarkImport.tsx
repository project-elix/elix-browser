import {useState} from 'react';
import {Upload} from 'lucide-react';
import {rpc} from '../browser/bridge';
import {parseBookmarkFile} from '../browser/bookmarkImport';
import type {Entry} from '../types';
export default function BookmarkImport({onChange,onError}:{onChange:()=>Promise<unknown>;onError:(e:unknown)=>void}){
 const[items,setItems]=useState<Entry[]|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState('');
 return <div className="bookmark-import"><label className="subtle"><Upload size={16}/>Importer bokmerker<input type="file" accept=".html,.htm,text/html" aria-label="Velg eksportert bokmerkefil" disabled={busy} onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(!file)return;if(file.size>10_000_000){onError('Maksimal filstørrelse er 10 MB');return;}void file.text().then(parseBookmarkFile).then(rows=>{setItems(rows);setMessage('')}).catch(onError)}}/></label><small>Eksporter bokmerkene som HTML fra Chrome, Edge, Firefox eller en annen nettleser.</small>{items&&<div className="notice">{items.length} gyldige nettadresser. Duplikater hoppes over. Mapper importeres som en flat liste.<button disabled={busy||!items.length} onClick={()=>{setBusy(true);void rpc<number>('import_bookmarks',{items}).then(async count=>{setItems(null);setMessage(`${count} nye bokmerker importert`);await onChange()}).catch(onError).finally(()=>setBusy(false))}}>Importer</button><button disabled={busy} onClick={()=>setItems(null)}>Avbryt</button></div>}{message&&<p role="status">{message}</p>}</div>
}
