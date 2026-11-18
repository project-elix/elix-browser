import {useState} from 'react';
import type {Session,Tab} from '../types';
import {rpc} from '../browser/bridge';

export function groupColor(session:Session,group:string,workspace=session.workspace||'Personlig'){
 const color=session.group_rules?.[workspace]?.colors[group];
 return color&&/^#[0-9a-f]{6}$/i.test(color)?color:undefined;
}

export function groupMark(session:Session,group:string,workspace?:string){
 const color=groupColor(session,group,workspace);
 return color?{borderInlineStart:`3px solid ${color}`}:undefined;
}

export default function GroupRules({profile,session,active,onError}:{profile:string;session:Session;active?:Tab;onError:(error:unknown)=>void}){
 const workspace=session.workspace||'Personlig',rules=session.group_rules?.[workspace];
 const [domain,setDomain]=useState(''),[group,setGroup]=useState(''),[color,setColor]=useState('#69a8ec'),[busy,setBusy]=useState(false);
 const action=async(operation:string,fields:Record<string,string>={})=>{
  setBusy(true);
  try{await rpc('group_rules',{profile,workspace,operation,...fields});if(operation==='save'){setDomain('');setGroup('')}}catch(e){onError(e)}finally{setBusy(false)}
 };
 return <details className="group-rules"><summary>Automatiske fanegrupper</summary>
  <form className="rail-workspace-form" onSubmit={e=>{e.preventDefault();void action('save',{domain,group,color})}}>
   <label className="rail-field">Domene og underdomener<input aria-label="Domene for fanegruppe" placeholder="example.com" value={domain} onChange={e=>setDomain(e.target.value)} maxLength={253}/></label>
   <label className="rail-field">Gruppenavn<input aria-label="Navn på regelgruppe" required maxLength={40} list="workspace-tab-groups" value={group} onChange={e=>{setGroup(e.target.value);const saved=groupColor(session,e.target.value);if(saved)setColor(saved)}}/></label>
   <label className="rail-field">Gruppefarge<input aria-label="Gruppefarge" type="color" value={color} onChange={e=>setColor(e.target.value)}/></label>
   <button className="subtle" disabled={busy||!group.trim()}>{domain.trim()?'Lagre regel':'Lagre gruppefarge'}</button>
  </form>
  {Object.entries(rules?.domains??{}).map(([host,name])=><div className="rule-row" key={host}>
   <button type="button" className="subtle" title="Rediger regel" disabled={busy} onClick={()=>{setDomain(host);setGroup(name);setColor(groupColor(session,name)||'#69a8ec')}}><i style={{backgroundColor:groupColor(session,name)}}/>{host} → {name}</button>
   <button className="subtle" disabled={busy} aria-label={`Fjern regel for ${host}`} onClick={()=>void action('remove',{domain:host})}>Fjern</button>
  </div>)}
  {!!Object.keys(rules?.domains??{}).length&&<button className="subtle" disabled={busy} onClick={()=>void action('apply')}>Kjør reglene på nytt</button>}
  {active&&(active.group_manual??Boolean(active.group))&&<button className="subtle" disabled={busy} onClick={()=>void action('resume',{tab_id:active.id})}>Bruk regler igjen for aktiv fane</button>}
 </details>;
}
