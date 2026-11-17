import {useEffect,useState} from 'react';
import {listen} from '@tauri-apps/api/event';
import {Camera,Mic,Shield,MapPin,Bell} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
interface SitePermission {id:string;tab_id:string;profile_id:string;origin:string;top_origin:string;kind:'camera'|'microphone'|'location'|'notifications'}
export default function SitePermissions({onError}:{onError:(e:unknown)=>void}) {
 const [requests,setRequests]=useState<SitePermission[]>([]),[busy,setBusy]=useState(false);
 useEffect(()=>{if(!desktop)return;let alive=true;let off:(()=>void)|undefined;
  void listen<SitePermission[]>('site-permissions',e=>{if(alive)setRequests(e.payload)}).then(unlisten=>{if(!alive)unlisten();else off=unlisten;return rpc<SitePermission[]>('site_permissions')}).then(items=>{if(alive)setRequests(items)}).catch(onError);
  return()=>{alive=false;off?.()};
 },[onError]);
 const item=requests[0];if(!item)return null;
 const respond=async(allow:boolean)=>{setBusy(true);try{await rpc('respond_permission',{id:item.id,allow});setRequests(await rpc<SitePermission[]>('site_permissions'))}catch(e){onError(e)}finally{setBusy(false)}};
 return <div className="permission-backdrop"><section className="permission-dialog" role="alertdialog" aria-modal="true" aria-labelledby="permission-heading" aria-describedby="permission-details"><Shield size={24}/><h2 id="permission-heading">Tillate {{camera:'kamera',microphone:'mikrofon',location:'posisjon',notifications:'varsler'}[item.kind]}?</h2><p id="permission-details"><strong>{item.origin}</strong> ber om tilgang til {{camera:'kamera',microphone:'mikrofon',location:'posisjon',notifications:'varsler'}[item.kind]}.</p>{item.top_origin!==item.origin&&<p>Forespørselen kommer fra innhold på <strong>{item.top_origin}</strong>.</p>}<p>Gjelder denne forespørselen. Du blir spurt igjen ved et nytt besøk. Windows sine personverninnstillinger gjelder også.</p><div className="permission-actions"><button autoFocus disabled={busy} onClick={()=>void respond(false)}>Blokker</button><button className="primary" disabled={busy} onClick={()=>void respond(true)}>{item.kind==='camera'?<Camera size={16}/>:item.kind==='microphone'?<Mic size={16}/>:item.kind==='location'?<MapPin size={16}/>:<Bell size={16}/>}Tillat</button></div></section></div>
}
