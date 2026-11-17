import {useState,type ReactNode} from 'react';
import {Search,Plus,X,Undo2,Columns2,Layers,Tags} from 'lucide-react';
import type {Session} from '../types';
import {rpc,internalTitle} from '../browser/bridge';
import {splitAction,splitCandidates,splitShortcut,reorderAction} from '../browser/tabTools';
import TabIcon from './TabIcon';

export default function TabTools({session,compact,searchOpen,onExpand,onSearch,onSplit,onError,children,playerTabId}:{playerTabId?:string|null;children?:ReactNode;session:Session;compact:boolean;searchOpen:boolean;onExpand:()=>void;onSearch:(v:boolean)=>void;onSplit:()=>void;onError:(e:unknown)=>void}){
 const[name,setName]=useState(''),[adding,setAdding]=useState(false),[query,setQuery]=useState('');
 const active=session.tabs.find(t=>t.id===session.active);
 const workspaces=session.workspaces?.length?session.workspaces:['Personlig'];
 const candidates=splitCandidates(session,playerTabId),canSplit=!!splitAction(session,playerTabId);
 const splitLabel=session.split?'Avslutt delt visning':'Åpne delt visning';
 const splitTitle=canSplit?`${splitLabel} · ${splitShortcut}`:active?.id===playerTabId?'Flytt minispilleren tilbake til fanen før du åpner delt visning':`Delt visning · ${splitShortcut} · Åpne to nettsider i samme arbeidsområde og privatmodus`;
 const group=(value:string)=>{const name=value.trim();if(!active||name===(active.group||''))return;const last=name?session.tabs.filter(t=>t.id!==active.id&&t.group===name&&t.pinned===active.pinned&&(t.workspace||'Personlig')===(active.workspace||'Personlig')).at(-1):undefined;const move=last&&reorderAction(session,active.id,last.id,'after');void rpc('group_tab',{id:active.id,group:name,workspace:active.workspace||'Personlig'}).then(()=>{if(move)return rpc('reorder_tab',move)}).catch(onError)};
 return <section className={`rail-tools ${compact?'rail-tools-compact':''}`} aria-label="Arbeidsområder og faner">
 {compact?<>
  <button className="rail-tool" title={`Arbeidsområde: ${session.workspace||'Personlig'}`} aria-label="Velg arbeidsområde" onClick={onExpand}><Layers size={19}/></button>
  <button className="rail-tool" title={active?.group?`Fanegruppe: ${active.group}`:'Fanegruppe'} aria-label="Endre fanegruppe" onClick={onExpand}><Tags size={19}/></button>
 </>:<>
  <div className="rail-section-heading"><h2>Arbeidsområde</h2><button title="Nytt arbeidsområde" aria-label="Nytt arbeidsområde" aria-expanded={adding} onClick={()=>setAdding(!adding)}><Plus size={16}/></button></div>
  <select aria-label="Arbeidsområde" value={session.workspace||'Personlig'} onChange={e=>void rpc('workspace',{name:e.target.value}).catch(onError)}>{workspaces.map(w=><option key={w}>{w}</option>)}</select>
  {adding&&<form className="rail-workspace-form" onSubmit={e=>{e.preventDefault();void rpc('workspace',{name:name.trim()}).then(()=>{setName('');setAdding(false)}).catch(onError)}}><input autoFocus aria-label="Navn på arbeidsområde" maxLength={40} value={name} onChange={e=>setName(e.target.value)} placeholder="Jobb, gaming …"/><div><button className="subtle" disabled={!name.trim()}>Opprett</button><button type="button" aria-label="Avbryt nytt arbeidsområde" onClick={()=>setAdding(false)}><X size={16}/></button></div></form>}
  <label className="rail-field">Gruppe for aktiv fane<input key={`${active?.id}-${active?.group}`} aria-label="Fanegruppe" maxLength={40} disabled={!active} placeholder="Ingen gruppe" list="workspace-tab-groups" defaultValue={active?.group||''} onBlur={e=>group(e.target.value)} onKeyDown={e=>{if(e.key==='Enter')e.currentTarget.blur()}}/></label><datalist id="workspace-tab-groups">{Array.from(new Set(session.tabs.filter(t=>(t.workspace||'Personlig')===(session.workspace||'Personlig')).map(t=>t.group).filter(Boolean))).map(group=><option key={group} value={group}/>)}</datalist>
  <label className="rail-field">Flytt aktiv fane<select aria-label="Flytt til arbeidsområde" disabled={!active} value={active?.workspace||'Personlig'} onChange={e=>{if(active)void rpc('group_tab',{id:active.id,group:active.group||'',workspace:e.target.value}).catch(onError)}}>{workspaces.map(w=><option key={w}>{w}</option>)}</select></label>
 </>}
 {children}
 <button className="rail-tool" onClick={onSplit} title={splitTitle} aria-label={splitLabel} aria-keyshortcuts="Control+Alt+S" aria-pressed={!!session.split} disabled={!canSplit}><Columns2 size={19}/><span>{splitLabel}</span></button>
 {!compact&&<>
  <select aria-label="Åpne delt visning med" title={`Velg siden ved siden av · ${splitShortcut} veksler delt visning`} value={session.split?.find(id=>id!==session.active)??''} disabled={!candidates.length} onChange={e=>{if(e.target.value)void rpc('split',{other:e.target.value}).catch(onError)}}><option value="">Velg andre side …</option>{candidates.map(t=><option key={t.id} value={t.id}>{t.title}</option>)}</select>
  <small className="rail-shortcut">Delt visning: {splitShortcut}</small>
 </>}
 {session.split&&!compact&&<div className="split-tabs" aria-label="Sider i delt visning">{session.split.map(id=>{const t=session.tabs.find(t=>t.id===id);return t&&<button key={id} title={t.title} className={id===session.active?'selected':''} onClick={()=>void rpc('activate',{id}).catch(onError)}><TabIcon tab={t}/><span>{t.title}</span></button>})}</div>}
 <button className="rail-tool" onClick={()=>void rpc('reopen_tab').catch(onError)} title="Gjenåpne lukket fane · Ctrl+Shift+T" aria-label="Gjenåpne lukket fane" disabled={!session.closed?.length}><Undo2 size={19}/><span>Gjenåpne fane</span></button>
 <button className="rail-tool" onClick={()=>onSearch(!searchOpen)} title="Søk i faner · Ctrl+Shift+A" aria-label="Søk i faner" aria-expanded={searchOpen} aria-controls="sidebar-tab-search"><Search size={19}/><span>Søk i faner</span></button>
 {searchOpen&&!compact&&<div className="tab-search" id="sidebar-tab-search"><div><input autoFocus aria-label="Søk blant alle åpne faner" placeholder="Alle arbeidsområder …" value={query} onChange={e=>setQuery(e.target.value)}/><button aria-label="Lukk fanesøk" onClick={()=>onSearch(false)}><X size={16}/></button></div><ul>{session.tabs.filter(t=>`${t.title} ${t.url} ${t.group??''} ${t.workspace??''}`.toLowerCase().includes(query.toLowerCase())).map(t=><li key={t.id}><button title={t.url} onClick={()=>void rpc('activate',{id:t.id}).then(()=>onSearch(false)).catch(onError)}><TabIcon tab={t}/><span>{t.url.startsWith('elix:')?internalTitle(t.url):t.title}<small>{t.workspace||'Personlig'} {t.group?`/ ${t.group}`:''} · {t.private?'Privat':t.url}</small></span></button></li>)}</ul></div>}
 </section>;
}
