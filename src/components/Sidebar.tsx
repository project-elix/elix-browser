import {Home,Gamepad2,Star,History,Download,Terminal,Settings,PanelLeftClose,PanelLeftOpen} from 'lucide-react';
import type {Appearance} from '../types';
const destinations=[{icon:Home,url:'start',name:'Startside'},{icon:Gamepad2,url:'gaming',name:'Gaming'},{icon:Star,url:'bookmarks',name:'Bokmerker'},{icon:History,url:'history',name:'Historikk'},{icon:Download,url:'downloads',name:'Nedlastinger'},{icon:Terminal,url:'developer',name:'Utvikler'}];
export default function Sidebar({mode,active,onGo,onMode,busy}:{mode:Appearance['sidebar'];active:string;onGo:(url:string)=>Promise<unknown>;onMode:(mode:Appearance['sidebar'])=>void;busy:boolean}){
 if(mode==='hidden')return null;
 const expanded=mode==='expanded';
 return <aside className={`rail ${expanded?'rail-expanded':''}`} aria-label="Sidemeny">
 <button className="rail-toggle" aria-label={expanded?'Vis bare ikoner':'Utvid sidemeny'} title={expanded?'Vis bare ikoner':'Utvid sidemeny'} aria-expanded={expanded} disabled={busy} onClick={()=>onMode(expanded?'icons':'expanded')}>{expanded?<PanelLeftClose size={20}/>:<PanelLeftOpen size={20}/>}<span>ELIX Browser</span></button>
 <nav aria-label="Hovedmeny">{destinations.map(({icon:Icon,url,name})=><button key={url} title={name} aria-label={name} aria-current={active===`elix://${url}`?'page':undefined} className={active===`elix://${url}`?'selected':''} onClick={()=>void onGo(`elix://${url}`)}><Icon size={20}/><span>{name}</span></button>)}</nav>
 <div className="rail-bottom"><button title="Innstillinger" aria-label="Innstillinger" aria-current={active==='elix://settings'?'page':undefined} className={active==='elix://settings'?'selected':''} onClick={()=>void onGo('elix://settings')}><Settings size={20}/><span>Innstillinger</span></button><small><i className="status-dot"/>Lokale brukerdata</small></div></aside>
}
