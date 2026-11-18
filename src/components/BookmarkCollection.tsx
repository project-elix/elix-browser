import {useEffect,useRef,useState,type KeyboardEvent,type MouseEvent} from 'react';
import {ChevronDown,ChevronRight,Folder,FolderPlus,MoreHorizontal,Star,X} from 'lucide-react';
import type {Entry} from '../types';
import {desktop,rpc} from '../browser/bridge';
import {listen} from '@tauri-apps/api/event';
import {bookmarkAddress,bookmarkGroups} from '../browser/bookmarks';
import BookmarkImport from './BookmarkImport';
import Favicon from './Favicon';
import './bookmarks.css';

type Target={kind:'bookmark';id:number}|{kind:'group';name:string}|{kind:'collection'};
type Editor={kind:'create';moveId?:number}|{kind:'rename';name:string}|{kind:'edit';id:number}|{kind:'move';id:number};
type Props={items:Entry[];groups:string[];profile:string;variant:'page'|'sidebar'|'toolbar'|'panel';onGo:(url:string)=>Promise<unknown>;onChange:()=>Promise<unknown>;onError:(e:unknown)=>void;onExpand?:()=>void};

export default function BookmarkCollection({items,groups,profile,variant,onGo,onChange,onError,onExpand}:Props){
 const [filter,setFilter]=useState(''),[expandedGroups,setExpandedGroups]=useState<Map<string,boolean>>(()=>new Map()),[target,setTarget]=useState<Target|null>(null),[editor,setEditor]=useState<Editor|null>(null);
 const [expandedSearchGroups,setExpandedSearchGroups]=useState<Map<string,boolean>>(()=>new Map());
 const [visibleGroups,setVisibleGroups]=filter.trim()?[expandedSearchGroups,setExpandedSearchGroups] as const:[expandedGroups,setExpandedGroups] as const;
 const [name,setName]=useState(''),[address,setAddress]=useState(''),[group,setGroup]=useState(''),[busy,setBusy]=useState(false),[message,setMessage]=useState('');
 const root=useRef<HTMLDivElement>(null),actions=useRef<HTMLDivElement>(null),opener=useRef<HTMLElement|null>(null),running=useRef(false),currentProfile=useRef(profile);
 currentProfile.current=profile;
 const selected=target?.kind==='bookmark'?items.find(item=>item.id===target.id):undefined;
 const sections=bookmarkGroups(items,groups,filter);
 const close=(restore=false)=>{setTarget(null);setEditor(null);if(restore)requestAnimationFrame(()=>opener.current?.focus())};
 useEffect(()=>{setTarget(null);setEditor(null);setExpandedGroups(new Map());setExpandedSearchGroups(new Map());setFilter('');setMessage('')},[profile]);
 useEffect(()=>{if(!desktop)return;let disposed=false;let unlisten:(()=>void)|undefined;void listen('page-focused',()=>{if(!running.current)close()}).then(stop=>{if(disposed)stop();else unlisten=stop}).catch(onError);return()=>{disposed=true;unlisten?.()}},[onError]);
 useEffect(()=>{if(!target&&!editor)return;const frame=requestAnimationFrame(()=>{actions.current?.scrollIntoView({block:'nearest'});actions.current?.querySelector<HTMLElement>(editor?'input,select':'[role="menuitem"]:not(:disabled)')?.focus()});const outside=(event:globalThis.PointerEvent)=>{if(!root.current?.contains(event.target as Node)&&!running.current)close()};const escape=(event:globalThis.KeyboardEvent)=>{if(event.key==='Escape'&&!running.current){event.preventDefault();event.stopPropagation();close(true)}};document.addEventListener('pointerdown',outside);document.addEventListener('keydown',escape,true);return()=>{cancelAnimationFrame(frame);document.removeEventListener('pointerdown',outside);document.removeEventListener('keydown',escape,true)}},[target,editor]);
 const context=(event:MouseEvent<HTMLElement>,value:Target)=>{event.preventDefault();event.stopPropagation();opener.current=event.currentTarget;onExpand?.();setEditor(null);setTarget(value);setMessage('')};
 const contextKey=(event:KeyboardEvent<HTMLElement>,value:Target)=>{if(event.key==='ContextMenu'||event.shiftKey&&event.key==='F10'){event.preventDefault();opener.current=event.currentTarget;onExpand?.();setEditor(null);setTarget(value)}};
 const run=async(action:()=>Promise<unknown>,success='')=>{if(running.current)return;running.current=true;setBusy(true);const actingProfile=profile;try{await action();if(currentProfile.current!==actingProfile)return;await onChange();if(currentProfile.current!==actingProfile)return;setMessage(success);close(true)}catch(error){if(currentProfile.current===actingProfile)onError(error)}finally{running.current=false;setBusy(false)}};
 const mutate=(action:string,data:Record<string,unknown>,success='')=>run(()=>rpc(action,{profile,...data}),success);
 const openPage=(url:string,newTab=false)=>{close();void (newTab?rpc('new_tab',{url}):onGo(url)).catch(onError)};
 const begin=(value:Editor)=>{setEditor(value);setName(value.kind==='rename'?value.name:value.kind==='edit'?items.find(item=>item.id===value.id)?.title||'':'');setAddress(value.kind==='edit'?items.find(item=>item.id===value.id)?.url||'':'');setGroup('id'in value?items.find(item=>item.id===value.id)?.group||'':'')};
 const submit=()=>{if(!editor)return;const value=editor;const trimmed=name.trim();if((value.kind==='create'||value.kind==='rename')&&(!trimmed||[...trimmed].length>80)){onError('Gruppenavnet må være mellom 1 og 80 tegn.');return}
  if(value.kind==='create')void run(async()=>{await rpc('create_bookmark_group',{profile,name:trimmed});if(value.moveId!==undefined)await rpc('move_bookmark',{profile,id:value.moveId,group:trimmed});if(currentProfile.current===profile)setExpandedGroups(previous=>new Map(previous).set(trimmed,true))},'Gruppen er opprettet.');
  if(value.kind==='rename')void mutate('rename_bookmark_group',{name:value.name,new_name:trimmed},'Gruppen har fått nytt navn.');
  if(value.kind==='move')void mutate('move_bookmark',{id:value.id,group},'Bokmerket er flyttet.');
  if(value.kind==='edit'){try{void mutate('edit_bookmark',{id:value.id,title:trimmed,url:bookmarkAddress(address)},'Bokmerket er oppdatert.')}catch(error){onError(error)}}
 };
 const keyMenu=(event:KeyboardEvent<HTMLDivElement>)=>{if(event.key==='Escape'){event.stopPropagation();if(!busy)close(true);return}if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key)||editor)return;const buttons=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')];if(!buttons.length)return;event.preventDefault();const index=buttons.indexOf(document.activeElement as HTMLButtonElement);const next=event.key==='Home'?0:event.key==='End'?buttons.length-1:event.key==='ArrowDown'?(index+1)%buttons.length:(index-1+buttons.length)%buttons.length;buttons[next].focus()};
 const entry=(item:Entry)=><div className="bookmark-item" key={item.id}>
  <button className="bookmark-open" title={`${item.title||item.url}\n${item.url}`} onClick={()=>openPage(item.url)} onContextMenu={e=>context(e,{kind:'bookmark',id:item.id})} onKeyDown={e=>contextKey(e,{kind:'bookmark',id:item.id})}><Favicon icon={item.favicon}/><span><strong>{item.title||item.url}</strong>{variant==='page'&&<small>{item.url}</small>}</span></button>
  {variant==='page'&&<button className="bookmark-more" aria-label={`Valg for ${item.title||item.url}`} title="Bokmerkevalg" onClick={e=>context(e,{kind:'bookmark',id:item.id})}><MoreHorizontal size={16}/></button>}
 </div>;
 return <div ref={root} className={`bookmark-collection bookmark-collection-${variant}`} onContextMenu={event=>{
  if(event.target instanceof Element&&event.target.closest('input,textarea,select,[contenteditable="true"]')){close();return}
  context(event,{kind:'collection'});
 }}>
  {variant==='page'&&<><span className="eyebrow">ELIX / BIBLIOTEK</span><div className="page-heading"><h1>Bokmerkene dine.</h1><button className="subtle" onClick={()=>begin({kind:'create'})}><FolderPlus size={16}/>Ny gruppe</button></div><p className="lead">Samle favorittene i grupper. Høyreklikk et bokmerke eller en gruppe for flere valg.</p><BookmarkImport onChange={onChange} onError={onError}/><input className="filter-input" aria-label="Søk i bokmerker" placeholder="Søk i bokmerker eller grupper …" value={filter} onChange={event=>{setFilter(event.target.value);setExpandedSearchGroups(new Map())}}/></>}
  <div className="bookmark-collection-list">{sections.map(section=>{
   if(!section.name)return section.items.length?<section className="bookmark-ungrouped" key="ungrouped">{variant==='page'&&groups.length>0&&<h2>Uten gruppe</h2>}{section.items.map(entry)}</section>:null;
   const expanded=visibleGroups.get(section.name)??(variant==='page');
   const toggle=()=>{onExpand?.();close();setVisibleGroups(previous=>new Map(previous).set(section.name,!expanded))};
   return <section className="bookmark-folder" key={`group-${section.name}`}>
    <button className="bookmark-folder-toggle" aria-label={`Bokmerkegruppe ${section.name}`} aria-expanded={expanded} onClick={toggle} onContextMenu={event=>context(event,{kind:'group',name:section.name})} onKeyDown={event=>contextKey(event,{kind:'group',name:section.name})}><Folder size={16}/><span>{section.name}</span><small>{section.items.length}</small>{expanded?<ChevronDown size={12}/>:<ChevronRight size={12}/>}</button>
    {expanded&&<div className="bookmark-folder-items">{section.items.length?section.items.map(entry):<p className="bookmark-empty">Gruppen er tom. Høyreklikk et bokmerke for å flytte det hit.</p>}</div>}
   </section>;
  })}</div>
  {!items.length&&!groups.length&&<p className="bookmark-empty"><Star size={15}/>Lagre en side med stjernen i adressefeltet.</p>}
  {filter&&!sections.some(section=>section.items.length||section.name)&&<p className="bookmark-empty">Ingen bokmerker eller grupper matcher søket.</p>}
  {(target||editor)&&<div ref={actions} className="bookmark-actions" onKeyDown={keyMenu} onContextMenu={event=>event.stopPropagation()}>
   <div className="bookmark-actions-heading"><strong>{editor?.kind==='create'?'Ny bokmerkegruppe':editor?.kind==='rename'?'Gi gruppen nytt navn':editor?.kind==='edit'?'Rediger bokmerke':editor?.kind==='move'?'Flytt bokmerke':selected?.title||selected?.url||(target?.kind==='group'?target.name:'Bokmerker')}</strong><button disabled={busy} aria-label="Lukk bokmerkevalg" onClick={()=>close(true)}><X size={15}/></button></div>
   {editor?<form aria-label="Rediger bokmerker" onSubmit={event=>{event.preventDefault();submit()}}>
    {editor.kind==='move'?<label>Gruppe<select aria-label="Flytt til bokmerkegruppe" value={group} disabled={busy} onChange={event=>setGroup(event.target.value)}><option value="">Uten gruppe</option>{groups.map(name=><option key={name} value={name}>{name}</option>)}</select></label>:<label>{editor.kind==='edit'?'Tittel':'Gruppenavn'}<input aria-label={editor.kind==='edit'?'Bokmerketittel':'Gruppenavn'} value={name} maxLength={editor.kind==='edit'?250:80} required={editor.kind!=='edit'} disabled={busy} onChange={event=>setName(event.target.value)}/></label>}
    {editor.kind==='edit'&&<label>Adresse<input aria-label="Bokmerkeadresse" type="url" required value={address} maxLength={8192} disabled={busy} onChange={event=>setAddress(event.target.value)}/></label>}
    <div className="bookmark-form-actions"><button className="primary" disabled={busy}>{busy?'Lagrer …':'Lagre'}</button><button type="button" className="subtle" disabled={busy} onClick={()=>close(true)}>Avbryt</button></div>
   </form>:<div role="menu" aria-label="Bokmerkevalg">
    {selected&&<><button role="menuitem" disabled={busy} onClick={()=>openPage(selected.url)}>Åpne</button><button role="menuitem" disabled={busy} onClick={()=>openPage(selected.url,true)}>Åpne i ny fane</button><button role="menuitem" disabled={busy} onClick={()=>begin({kind:'edit',id:selected.id})}>Rediger bokmerke</button><button role="menuitem" disabled={busy} onClick={()=>begin({kind:'move',id:selected.id})}>Flytt til gruppe …</button><button role="menuitem" disabled={busy} onClick={()=>begin({kind:'create',moveId:selected.id})}>Ny gruppe med bokmerket …</button><button className="bookmark-danger" role="menuitem" disabled={busy} onClick={()=>void mutate('remove_bookmark',{id:selected.id},'Bokmerket er fjernet.')}>Fjern bokmerke</button></>}
    {target?.kind==='group'&&<><button role="menuitem" disabled={busy} onClick={()=>begin({kind:'rename',name:target.name})}>Gi gruppen nytt navn</button><button role="menuitem" disabled={busy} onClick={()=>void mutate('remove_bookmark_group',{name:target.name},'Gruppen er fjernet. Bokmerkene ligger under Uten gruppe.')}>Fjern gruppe – behold bokmerker</button></>}
    {!selected&&<button role="menuitem" disabled={busy} onClick={()=>begin({kind:'create'})}>Ny bokmerkegruppe</button>}
   </div>}
  </div>}
  {message&&<p className="bookmark-status" role="status">{message}</p>}
 </div>;
}
