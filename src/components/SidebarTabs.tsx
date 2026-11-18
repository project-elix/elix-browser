import {useState,useRef,useEffect,type PointerEvent,type MouseEvent} from 'react';
import {GripVertical,Pin,ShieldCheck,ChevronDown,ChevronRight,X} from 'lucide-react';
import type {Session,Tab} from '../types';
import {internalTitle,rpc} from '../browser/bridge';
import {reorderAction} from '../browser/tabTools';
import type {TabDropTarget} from '../browser/useTabDrag';
import TabIcon from './TabIcon';
import {groupMark} from './GroupRules';

type Props={vertical?:boolean;onTabMenu?:(id:string)=>void;session:Session;held?:string|null;dragging:string|null;dropTarget:TabDropTarget|null;onPointerDown:(event:PointerEvent<HTMLElement>,id:string)=>void;onPointerMove:(event:PointerEvent<HTMLElement>)=>void;onPointerUp:(event:PointerEvent<HTMLElement>)=>void;onPointerCancel:()=>void;onLostPointerCapture?:(event:PointerEvent<HTMLElement>)=>void;onClickCapture:(event:MouseEvent<HTMLElement>)=>void;onError:(error:unknown)=>void};

export default function SidebarTabs({vertical,onTabMenu,session,held,dragging,dropTarget,onPointerDown,onPointerMove,onPointerUp,onPointerCancel,onLostPointerCapture,onClickCapture,onError}:Props){
 const tabs=session.tabs.filter(t=>(t.workspace||'Personlig')===(session.workspace||'Personlig'));
 const [collapsed,setCollapsed]=useState<Set<string>>(()=>new Set());
 const [focusMove,setFocusMove]=useState<{id:string;workspace:string;pinned:boolean;order:string[]}|null>(null);
 const buttons=useRef(new Map<string,HTMLButtonElement>());
 useEffect(()=>{if(!focusMove)return;if((session.workspace||'Personlig')!==focusMove.workspace||!session.tabs.some(t=>t.id===focusMove.id)){setFocusMove(null);return}const current=session.tabs.filter(t=>(t.workspace||'Personlig')===focusMove.workspace&&t.pinned===focusMove.pinned).map(t=>t.id);if(current.length===focusMove.order.length&&current.every((id,index)=>id===focusMove.order[index])){buttons.current.get(focusMove.id)?.focus();setFocusMove(null)}},[session.tabs,session.workspace,focusMove]);
 const sections=tabs.reduce<Array<{key:string;group:string;tabs:Tab[]}>>((all,tab)=>{const key=`${session.workspace||'Personlig'}:${tab.pinned}:${tab.group||''}`;const last=all.at(-1);if(last?.key===key)last.tabs.push(tab);else all.push({key,group:tab.group||'',tabs:[tab]});return all},[]);
 return <section className="sidebar-tabs" aria-label="Faner i arbeidsområdet">
  <div className="rail-section-heading"><h2>Faner</h2><span>{tabs.length}</span></div>
  <small>Dra for å sortere. Alt+↑/↓ flytter en markert fane.</small>
  {sections.map((section,index)=><div className="sidebar-tab-group" key={`${section.key}:${index}`}>{section.group&&<button data-tab-group-anchor={section.tabs[0].id} style={groupMark(session,section.group)} className={`sidebar-group-heading ${section.tabs.some(t=>t.id===session.active)?'active':''} ${dropTarget?.edge==='group'&&dropTarget.id===section.tabs[0].id?'group-drop-target':''}`} aria-expanded={!collapsed.has(section.key)} onClick={()=>setCollapsed(previous=>{const next=new Set(previous);if(next.has(section.key))next.delete(section.key);else next.add(section.key);return next})}>{collapsed.has(section.key)?<ChevronRight size={13}/>:<ChevronDown size={13}/>}<span>{section.group}</span><small>{section.tabs.length}</small></button>}
  {(!section.group||!collapsed.has(section.key))&&<ul>{section.tabs.map(tab=>{const title=tab.url.startsWith('elix:')?internalTitle(tab.url):tab.title;return <li key={tab.id} data-tab-id={tab.id} data-tab-axis="vertical" className={`${tab.id===session.active?'selected':''} ${held===tab.id?'held':''} ${dragging===tab.id?'dragging':''} ${dropTarget?.id===tab.id?`drop-${dropTarget.edge}`:''}`} onContextMenu={onTabMenu?e=>{e.preventDefault();onTabMenu(tab.id)}:undefined} onDragStart={e=>e.preventDefault()} onPointerDown={e=>onPointerDown(e,tab.id)} onPointerMove={onPointerMove} onPointerUp={onPointerUp} onPointerCancel={onPointerCancel} onLostPointerCapture={onLostPointerCapture} onClickCapture={onClickCapture}>
   <button ref={element=>{if(element)buttons.current.set(tab.id,element);else buttons.current.delete(tab.id)}} aria-label={`Åpne ${title}`} aria-current={tab.id===session.active?'page':undefined} title={`${title}${tab.group?` · ${tab.group}`:''}${tab.pinned?' · Festet':''}${tab.private?' · Privat':''} · Dra eller bruk Alt+↑/↓ for å sortere`} onClick={()=>void rpc('activate',{id:tab.id}).catch(onError)} onKeyDown={e=>{if(!e.altKey||e.ctrlKey||e.metaKey||e.shiftKey||!['ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();const partition=tabs.filter(t=>t.pinned===tab.pinned);const index=partition.findIndex(t=>t.id===tab.id);const up=e.key==='ArrowUp',target=partition[index+(up?-1:1)];const action=target&&reorderAction(session,tab.id,target.id,up?'before':'after');if(action){const order=partition.filter(t=>t.id!==tab.id).map(t=>t.id);order.splice(action.before?order.indexOf(action.before):order.length,0,tab.id);setFocusMove({id:tab.id,workspace:tab.workspace||'Personlig',pinned:tab.pinned,order});void rpc('reorder_tab',action).catch(error=>{setFocusMove(null);onError(error)})}}}><GripVertical size={13} className="tab-grip"/><TabIcon tab={tab}/><span>{title}{tab.group&&<small>{tab.group}</small>}</span>{tab.pinned&&<Pin size={12}/>} {tab.private&&<ShieldCheck size={12}/>}</button>
   {vertical&&!tab.pinned&&<button className="tab-close vertical-tab-close" aria-label={`Lukk ${title}`} onClick={()=>void rpc('close_tab',{id:tab.id}).catch(onError)}><X size={14}/></button>}
  </li>})}</ul>}</div>)}
  {tabs.some(tab=>tab.id===dragging&&tab.group)&&<div data-tab-ungroup className={`tab-ungroup-target ${dropTarget?.edge==='ungroup'?'targeted':''}`}>Dra hit for å ta fanen ut av gruppen</div>}
 </section>;
}
