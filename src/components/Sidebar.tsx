import Favicon from "./Favicon";
import {Home,Gamepad2,Star,History,Download,Terminal,Settings,PanelLeftClose,PanelLeftOpen,Globe} from 'lucide-react';
import type {Appearance,Entry} from '../types';
const destinations=[{icon:Home,url:'start',name:'Startside'},{icon:Gamepad2,url:'gaming',name:'Gaming'},{icon:Star,url:'bookmarks',name:'Bokmerker'},{icon:History,url:'history',name:'Logg'},{icon:Download,url:'downloads',name:'Nedlastinger'},{icon:Terminal,url:'developer',name:'Utvikler'}];
export default function Sidebar({mode,active,onGo,onMode,busy,bookmarks,showBookmarks}:{mode:Appearance['sidebar'];active:string;onGo:(url:string)=>Promise<unknown>;onMode:(mode:Appearance['sidebar'])=>void;busy:boolean;bookmarks:Entry[];showBookmarks:boolean}){
 if(mode==='hidden')return null;
 const expanded=mode==='expanded';
 return <aside className={`rail ${expanded?'rail-expanded':''}`} aria-label="Sidemeny">
 <button className="rail-brand" aria-label="ELIX Browser – startside" onClick={()=>void onGo('elix://start')}><img src="/elix-mark.png" alt="ELIX"/><span>Browser</span></button>
 <button className="rail-toggle" aria-label={expanded?'Vis bare ikoner':'Utvid sidemeny'} title={expanded?'Vis bare ikoner':'Utvid sidemeny'} aria-expanded={expanded} disabled={busy} onClick={()=>onMode(expanded?'icons':'expanded')}>{expanded?<PanelLeftClose size={20}/>:<PanelLeftOpen size={20}/>}<span>Sidemeny</span></button>
 <nav aria-label="Hovedmeny">{destinations.map(({icon:Icon,url,name})=><button key={url} title={name} aria-label={name} aria-current={active===`elix://${url}`?'page':undefined} className={active===`elix://${url}`?'selected':''} onClick={()=>void onGo(`elix://${url}`)}><Icon size={20}/><span>{name}</span></button>)}</nav>
 {showBookmarks&&<section className="rail-bookmarks" aria-label="Bokmerker i sidemenyen"><h2>Bokmerker</h2>{bookmarks.length?bookmarks.map(b=><button key={b.id} title={b.title||b.url} aria-label={b.title||b.url} onClick={()=>void onGo(b.url)}><Favicon icon={b.favicon}/><span>{b.title||b.url}</span></button>):<p>Bruk stjernen for å lagre en side.</p>}</section>}
 <div className="rail-bottom"><button title="Innstillinger" aria-label="Innstillinger" aria-current={active==='elix://settings'?'page':undefined} className={active==='elix://settings'?'selected':''} onClick={()=>void onGo('elix://settings')}><Settings size={20}/><span>Innstillinger</span></button><small><i className="status-dot"/>Lokale brukerdata</small></div></aside>
}
