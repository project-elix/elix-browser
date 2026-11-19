import {useEffect,useState,type ReactNode} from 'react';
import {Search,Plus,X,Undo2,Columns2,Layers,Tags,ChevronDown} from 'lucide-react';
import type {Session} from '../types';
import {rpc} from '../browser/bridge';
import {splitAction,splitCandidates,splitShortcut} from '../browser/tabTools';
import TabIcon from './TabIcon';
import GroupRules from './GroupRules';
import SavedSessions from './SavedSessions';
import TabSearch from './TabSearch';

type ToolSection='split'|'tabs'|'workspace'|'sessions';
export default function TabTools({profile,session,searchOpen,onSearch,onSplit,onError,playerTabId,children}:{profile:string;playerTabId?:string|null;children?:ReactNode;session:Session;compact?:boolean;searchOpen:boolean;onExpand?:()=>void;onSearch:(v:boolean)=>void;onSplit:()=>void;onError:(e:unknown)=>void}){
 const [section,setSection]=useState<ToolSection|null>(searchOpen?'tabs':null);
 const [name,setName]=useState(''),[adding,setAdding]=useState(false);
 const active=session.tabs.find(t=>t.id===session.active);
 const workspaces=session.workspaces?.length?session.workspaces:['Personlig'];
 const candidates=splitCandidates(session,playerTabId),canSplit=!!splitAction(session,playerTabId);
 const splitLabel=session.split?'Avslutt delt visning':'Åpne delt visning';
 const splitTitle=canSplit?`${splitLabel} · ${splitShortcut}`:active?.id===playerTabId?'Flytt minispilleren tilbake til fanen før du åpner delt visning':`Delt visning · ${splitShortcut} · Åpne to nettsider i samme arbeidsområde og privatmodus`;
 useEffect(()=>{if(searchOpen)setSection('tabs')},[searchOpen]);
 const toggle=(next:ToolSection)=>{if(next!=='tabs'||section==='tabs')onSearch(false);setSection(section===next?null:next)};
 const group=(value:string)=>{
  const name=value.trim();
  if(active&&name!==(active.group||'')){
   void rpc('group_tab',{id:active.id,group:name,workspace:active.workspace||'Personlig'}).catch(onError);
  }
 };
 return <section className="rail-tools gear-tools" aria-label="Hurtigvalg for nettleseren">
  <button data-split-drop="" className="gear-section-toggle" aria-expanded={section==='split'} aria-controls="gear-split" onClick={()=>toggle('split')}><Columns2 size={18}/><span>Delt visning</span><ChevronDown size={14}/></button>
  {section==='split'&&<div className="gear-section-content" id="gear-split">
   <button data-split-drop="" className="rail-tool" onClick={onSplit} title={splitTitle} aria-label={splitLabel} aria-keyshortcuts="Control+Alt+S" aria-pressed={!!session.split} disabled={!canSplit}><Columns2 size={17}/><span>{splitLabel}</span></button>
   <select aria-label="Åpne delt visning med" title={`Velg siden ved siden av · ${splitShortcut} veksler delt visning`} value={session.split?.find(id=>id!==session.active)??''} disabled={!candidates.length} onChange={e=>{if(e.target.value)void rpc('split',{other:e.target.value}).catch(onError)}}><option value="">Velg andre side …</option>{candidates.map(t=><option key={t.id} value={t.id}>{t.title}</option>)}</select>
   <small className="rail-shortcut">{canSplit?`Snarvei: ${splitShortcut}`:'Åpne to nettsider i samme arbeidsområde og privatmodus.'}</small>
   {session.split&&<div className="split-tabs" aria-label="Sider i delt visning">{session.split.map(id=>{const t=session.tabs.find(t=>t.id===id);return t&&<button key={id} title={t.title} className={id===session.active?'selected':''} onClick={()=>void rpc('activate',{id}).catch(onError)}><TabIcon tab={t}/><span>{t.title}</span></button>})}</div>}
  </div>}
  <button className="gear-section-toggle" aria-expanded={section==='tabs'} aria-controls="gear-tabs" onClick={()=>toggle('tabs')}><Tags size={18}/><span>Faner</span><ChevronDown size={14}/></button>
  {section==='tabs'&&<div className="gear-section-content" id="gear-tabs">
   <button className="rail-tool" onClick={()=>onSearch(!searchOpen)} title="Søk i faner · Ctrl+Shift+A" aria-label="Søk i faner" aria-expanded={searchOpen} aria-controls="sidebar-tab-search"><Search size={17}/><span>Søk i faner</span></button>
   {searchOpen&&<TabSearch session={session} onClose={()=>onSearch(false)} onError={onError}/>}
   <button className="rail-tool" onClick={()=>void rpc('reopen_tab').catch(onError)} title="Gjenåpne lukket fane · Ctrl+Shift+T" aria-label="Gjenåpne lukket fane" disabled={!session.closed?.length}><Undo2 size={17}/><span>Gjenåpne fane</span></button>
   <form className="rail-group-form" key={`${active?.id}-${active?.group}`} onSubmit={event=>{event.preventDefault();group(String(new FormData(event.currentTarget).get('group')??''))}}><label className="rail-field">Gruppe for aktiv fane<input name="group" aria-label="Fanegruppe" maxLength={40} disabled={!active} placeholder="Ingen gruppe" list="workspace-tab-groups" defaultValue={active?.group||''}/></label><button type="submit" className="subtle" aria-label="Lagre fanegruppe" disabled={!active}>Lagre</button></form><datalist id="workspace-tab-groups">{Array.from(new Set(session.tabs.filter(t=>(t.workspace||'Personlig')===(session.workspace||'Personlig')).map(t=>t.group).filter(Boolean))).map(group=><option key={group} value={group}/>)}</datalist>
   <GroupRules key={session.workspace} profile={profile} session={session} active={active} onError={onError}/>
   <label className="rail-field">Flytt aktiv fane<select aria-label="Flytt til arbeidsområde" disabled={!active} value={active?.workspace||'Personlig'} onChange={e=>{if(active)void rpc('group_tab',{id:active.id,group:active.group||'',workspace:e.target.value}).catch(onError)}}>{workspaces.map(w=><option key={w}>{w}</option>)}</select></label>
  </div>}
  <button className="gear-section-toggle" aria-expanded={section==='workspace'} aria-controls="gear-workspace" onClick={()=>toggle('workspace')}><Layers size={18}/><span>Arbeidsområde</span><ChevronDown size={14}/></button>
  {section==='workspace'&&<div className="gear-section-content" id="gear-workspace">
   {children}
   <label className="rail-field">Aktivt arbeidsområde<select aria-label="Arbeidsområde" value={session.workspace||'Personlig'} onChange={e=>void rpc('workspace',{name:e.target.value}).catch(onError)}>{workspaces.map(w=><option key={w}>{w}</option>)}</select></label>
   <button className="rail-tool" title="Nytt arbeidsområde" aria-label="Nytt arbeidsområde" aria-expanded={adding} onClick={()=>setAdding(!adding)}><Plus size={17}/><span>Nytt arbeidsområde</span></button>
   {adding&&<form className="rail-workspace-form" onSubmit={e=>{e.preventDefault();void rpc('workspace',{name:name.trim()}).then(()=>{setName('');setAdding(false)}).catch(onError)}}><input autoFocus aria-label="Navn på arbeidsområde" maxLength={40} value={name} onChange={e=>setName(e.target.value)} placeholder="Jobb, gaming …"/><div><button className="subtle" disabled={!name.trim()}>Opprett</button><button type="button" aria-label="Avbryt nytt arbeidsområde" onClick={()=>setAdding(false)}><X size={16}/></button></div></form>}
  </div>}
  <button className="gear-section-toggle" aria-expanded={section==='sessions'} aria-controls="gear-sessions" onClick={()=>toggle('sessions')}><Layers size={18}/><span>Lagrede økter</span><ChevronDown size={14}/></button>
  {section==='sessions'&&<SavedSessions key={profile} profile={profile} onError={onError}/>}
 </section>;
}
