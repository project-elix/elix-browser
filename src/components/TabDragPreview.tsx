import {GripVertical,Pin} from 'lucide-react';
import {internalTitle} from '../browser/bridge';
import {dragPreviewPosition} from '../browser/tabDrag';
import type {TabDragPreview as DragPreview} from '../browser/useTabDrag';
import type {Tab} from '../types';
import TabIcon from './TabIcon';
import './tab-drag.css';

export default function TabDragPreview({drag,tabs}:{drag:DragPreview|null;tabs:Tab[]}) {
 if(!drag)return null;
 const tab=tabs.find(tab=>tab.id===drag.id);if(!tab)return null;
 const title=tab.url.startsWith('elix:')?internalTitle(tab.url):tab.title;
 const native=document.querySelector('.native-view')?.getBoundingClientRect();
 const position=dragPreviewPosition(drag.point,window.innerWidth,window.innerHeight,native);
 const target=tabs.find(tab=>tab.id===drag.target?.id);
 const targetTitle=target?.url.startsWith('elix:')?internalTitle(target.url):target?.title;
 return <>
  <div className={`tab-drag-preview ${position.docked?'tab-drag-docked':''}`} style={{left:position.left,top:position.top,width:position.width}} aria-hidden="true">
   <GripVertical size={15}/><TabIcon tab={tab}/><span><strong>{title}</strong><small>{target?`Slipp ${drag.target?.edge==='before'?'før':'etter'} ${targetTitle}`:'Flytter fane · Esc avbryter'}</small></span>{tab.pinned&&<Pin size={13}/>}
  </div>
  <span className="tab-drag-announcement" role="status" aria-live="polite">Flytter {title}. {target?`Slipp ${drag.target?.edge==='before'?'før':'etter'} ${targetTitle}.`:'Trykk Escape for å avbryte.'}</span>
 </>;
}
