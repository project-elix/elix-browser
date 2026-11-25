import {useEffect,useRef,useState,type KeyboardEvent,type PointerEvent,type ReactNode} from 'react';
import {PanelLeftClose,PanelRightClose,Plus} from 'lucide-react';
import type {TabLayout} from '../types';

export const defaultTabLayout:TabLayout={vertical:false,side:'left',width:240};
const clamp=(width:number)=>Math.max(160,Math.min(420,Math.round(width)));

export function TabLayoutSettings({value,onSave,onError}:{value:TabLayout;onSave:(next:TabLayout)=>Promise<void>;onError:(e:unknown)=>void}){
 const [draft,setDraft]=useState(value),[busy,setBusy]=useState(false);
 useEffect(()=>setDraft(value),[value.vertical,value.side,value.width]);
 async function save(){
  setBusy(true);
  try{await onSave(draft)}catch(error){onError(error)}finally{setBusy(false)}
 }
 return <section className="settings-section">
  <h2>Faneoppsett</h2>
  <fieldset disabled={busy}><legend className="sr-only">Faneoppsett for aktiv profil</legend>
   <label className="setting-row"><span>Vertikale faner</span><input type="checkbox" role="switch" checked={draft.vertical} onChange={e=>setDraft({...draft,vertical:e.target.checked})}/></label>
   <div className="appearance-grid">
    <label>Plassering av faner<select value={draft.side} onChange={e=>setDraft({...draft,side:e.target.value as TabLayout['side']})}><option value="left">Venstre</option><option value="right">Høyre</option></select></label>
    <label>Bredde på fanelisten · {draft.width} px<input type="range" aria-label="Bredde på fanelisten" min="160" max="420" value={draft.width} onChange={e=>setDraft({...draft,width:Number(e.target.value)})}/></label>
   </div>
   <button className="subtle" onClick={()=>void save()} disabled={draft.vertical===value.vertical&&draft.side===value.side&&draft.width===value.width}>Lagre faneoppsett</button>
  </fieldset>
 </section>;
}

export function VerticalTabRail({value,workspace,onNewTab,onSave,onError,children}:{value:TabLayout;workspace:string;onNewTab:()=>void;onSave:(next:TabLayout)=>Promise<void>;onError:(e:unknown)=>void;children:ReactNode}){
 const [width,setWidth]=useState<number|null>(null);
 const drag=useRef<{pointer:number;x:number;width:number}|null>(null);
 const direction=value.side==='left'?1:-1;
 const save=(patch:Partial<TabLayout>)=>void onSave({...value,...patch}).catch(onError);
 function start(e:PointerEvent<HTMLElement>){
  if(!e.isPrimary||e.button!==0)return;
  e.currentTarget.setPointerCapture(e.pointerId);
  drag.current={pointer:e.pointerId,x:e.clientX,width:value.width};
 }
 function resize(e:PointerEvent<HTMLElement>,finish:boolean){
  const current=drag.current;
  if(!current||e.pointerId!==current.pointer)return;
  const next=clamp(current.width+direction*(e.clientX-current.x));
  if(!finish){setWidth(next);return}
  drag.current=null;
  if(next!==value.width)save({width:next});
  setWidth(null);
 }
 function cancel(){drag.current=null;setWidth(null)}
 function key(e:KeyboardEvent<HTMLElement>){
  if(e.key==='Escape'){cancel();return}
  if(!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;
  e.preventDefault();
  save({width:e.key==='Home'?160:e.key==='End'?420:clamp(value.width+(e.key==='ArrowLeft'?-1:1)*direction*10)});
 }
 return <aside className={`vertical-tab-rail rail-${value.side}`} aria-label="Vertikale faner" style={{width:width??value.width}}>
  <div className="vertical-tab-heading"><div className="vertical-tab-heading-label"><h2>Faner</h2><span title={workspace}>{workspace}</span></div><button title="Vis fanene øverst" aria-label="Skjul vertikale faner" onClick={()=>save({vertical:false})}>{value.side==='left'?<PanelLeftClose size={17}/>:<PanelRightClose size={17}/>}</button></div>
  <button className="vertical-new-tab" aria-label="Åpne ny fane i fanelisten" title="Ny fane · Ctrl+T" onClick={onNewTab}><Plus aria-hidden="true" size={16}/><span>Ny fane</span></button>
  <div className="rail-scroll">{children}</div>
  <footer className="vertical-tab-footer">Alt + ↑ / ↓ for å sortere</footer>
  <div role="separator" aria-label="Endre bredde på fanelisten" aria-orientation="vertical" aria-valuemin={160} aria-valuemax={420} aria-valuenow={width??value.width} tabIndex={0} className="tab-width-handle" onKeyDown={key}
   onPointerDown={start} onPointerMove={e=>resize(e,false)} onPointerUp={e=>resize(e,true)} onPointerCancel={cancel} onLostPointerCapture={cancel}/>
 </aside>;
}
