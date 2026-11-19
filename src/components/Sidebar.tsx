import BookmarkCollection from './BookmarkCollection';
import {useCallback,useEffect,useRef,useState,type ReactNode} from 'react';
import {Home,Gamepad2,Star,History,Download,Terminal,Settings,PanelLeftClose,PanelLeftOpen,X} from 'lucide-react';
import type {Appearance,Entry} from '../types';
import {listen,type UnlistenFn} from '@tauri-apps/api/event';
import {desktop} from '../browser/bridge';
import './gear-menu.css';
const destinations=[{icon:Home,url:'start',name:'Startside'},{icon:Gamepad2,url:'gaming',name:'Gaming'},{icon:Star,url:'bookmarks',name:'Bokmerker'},{icon:History,url:'history',name:'Logg'},{icon:Download,url:'downloads',name:'Nedlastinger'},{icon:Terminal,url:'developer',name:'Utvikler'}];
type Props={mode:Appearance['sidebar'];active:string;onGo:(url:string)=>Promise<unknown>;onMode:(mode:Appearance['sidebar'])=>void;busy:boolean;bookmarks:Entry[];bookmarkGroups:string[];profile:string;onBookmarksChange:()=>Promise<unknown>;onError:(error:unknown)=>void;showBookmarks:boolean;children?:ReactNode;quickTools?:ReactNode;searchOpen?:boolean;onCloseTools?:()=>void};
export default function Sidebar({mode,active,onGo,onMode,busy,bookmarks,bookmarkGroups,profile,onBookmarksChange,onError,showBookmarks,children,quickTools,searchOpen=false,onCloseTools}:Props){
 const [open,setOpen]=useState(false);
 const menuRef=useRef<HTMLDivElement>(null),gearRef=useRef<HTMLButtonElement>(null);
 const closeToolsRef=useRef(onCloseTools);closeToolsRef.current=onCloseTools;
 const close=useCallback((restoreFocus=false)=>{setOpen(false);closeToolsRef.current?.();if(restoreFocus)gearRef.current?.focus()},[]);
 const previousActive=useRef(active);
 useEffect(()=>{if(previousActive.current!==active){previousActive.current=active;close()}},[active,close]);
 useEffect(()=>{if(searchOpen)setOpen(true)},[searchOpen]);
 useEffect(()=>{if(mode==='hidden')close()},[mode,close]);
 useEffect(()=>{
  if(!desktop)return;
  let disposed=false,unlisten:UnlistenFn|undefined;
  // Native focus events cross the WebView boundary asynchronously. A click on
  // the gear can already have returned focus to the shell when an old one arrives.
  void listen('page-focused',()=>{if(!document.hasFocus())close()}).then(stop=>{if(disposed)stop();else unlisten=stop}).catch(onError);
  return()=>{disposed=true;unlisten?.()};
 },[close,onError]);
 useEffect(()=>{
  if(!open)return;
  const outside=(event:PointerEvent)=>{if(!menuRef.current?.contains(event.target as Node))close()};
  const escape=(event:KeyboardEvent)=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close(true)}};
  document.addEventListener('pointerdown',outside);document.addEventListener('keydown',escape,true);
  return()=>{document.removeEventListener('pointerdown',outside);document.removeEventListener('keydown',escape,true)};
 },[open,close]);
 if(mode==='hidden')return null;
 const expanded=mode==='expanded',visuallyExpanded=expanded||open;
 const openSettings=()=>{close();void onGo('elix://settings')};
 return <aside className={`rail ${visuallyExpanded?'rail-expanded':''} ${open?'rail-quick-open':''} ${open&&!expanded?'rail-quick-temporary':''}`} aria-label="Sidemeny">
  <button className="rail-brand" aria-label="ELIX Browser – startside" onClick={()=>void onGo('elix://start')}><img src="/elix-mark.png" alt="ELIX"/><span>Browser</span></button>
  <button className="rail-toggle" aria-label={expanded?'Vis bare ikoner':'Utvid sidemeny'} title={expanded?'Vis bare ikoner':'Utvid sidemeny'} aria-expanded={expanded} disabled={busy} onClick={()=>{close();onMode(expanded?'icons':'expanded')}}>{expanded?<PanelLeftClose size={20}/>:<PanelLeftOpen size={20}/>}<span>Sidemeny</span></button>
  <div className="rail-scroll"><nav aria-label="Hovedmeny">{destinations.map(({icon:Icon,url,name})=><button key={url} title={name} aria-label={name} aria-current={active===`elix://${url}`?'page':undefined} className={active===`elix://${url}`?'selected':''} onClick={()=>void onGo(`elix://${url}`)}><Icon size={20}/><span>{name}</span></button>)}</nav>
   {children}
   {showBookmarks&&<section className="rail-bookmarks" aria-label="Bokmerker i sidemenyen"><h2>Bokmerker</h2><BookmarkCollection items={bookmarks} groups={bookmarkGroups} variant="sidebar" profile={profile} onGo={onGo} onChange={onBookmarksChange} onError={onError} onExpand={mode==='icons'?()=>onMode('expanded'):undefined}/></section>}
  </div>
  <div className="rail-bottom" ref={menuRef}>
   {open&&<div className="gear-panel" id="sidebar-quick-settings" role="region" aria-label="Hurtigvalg">
    <div className="gear-heading"><strong>Hurtigvalg</strong><button title="Lukk hurtigvalg" aria-label="Lukk hurtigvalg" onClick={()=>close(true)}><X size={15}/></button></div>
    {quickTools}
    <button className="gear-settings" onClick={openSettings}><Settings size={18}/><span>Innstillinger</span></button>
   </div>}
   <button ref={gearRef} className={`gear-trigger ${open||active==='elix://settings'?'selected':''}`} title="Hurtigvalg · dobbeltklikk for innstillinger" aria-label="Innstillinger og hurtigvalg" aria-expanded={open} aria-controls="sidebar-quick-settings" onClick={event=>{if(event.detail>1)return;if(open)close();else{setOpen(true);if(event.detail===0)requestAnimationFrame(()=>menuRef.current?.querySelector<HTMLButtonElement>('.gear-section-toggle')?.focus())}}} onKeyDown={event=>{if(event.key==='ArrowUp'||event.key==='ArrowDown'){event.preventDefault();setOpen(true);requestAnimationFrame(()=>menuRef.current?.querySelector<HTMLButtonElement>('.gear-section-toggle')?.focus())}}} onDoubleClick={openSettings}><Settings size={20}/><span>Innstillinger</span></button>
   <small className="rail-data-status"><i className="status-dot"/>Lokale brukerdata</small>
  </div>
 </aside>
}
