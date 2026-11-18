import {useCallback,useEffect,useRef,useState,type MouseEvent as ReactMouseEvent,type PointerEvent as ReactPointerEvent} from 'react';
import type {Session} from '../types';
import {rpc} from './bridge';
import {reorderAction} from './tabTools';
import {edgeScroll,groupDropTarget,type Point} from './tabDrag';

export type TabDropTarget=
 | {id:string;edge:'before'|'after'}
 | {id:string;edge:'group';group:string;workspace:string};
export type TabDragPreview={id:string;point:Point;target:TabDropTarget|null};
type Drag={id:string;pointerId:number;start:Point;point:Point;active:boolean;capture:Element;workspace:string;pinned:boolean};
type Callbacks={onError:(error:unknown)=>void;onNotice:(notice:string)=>void};

export function useTabDrag(session:Session,callbacks:Callbacks) {
 const current=useRef({session,callbacks});current.current={session,callbacks};
 const drag=useRef<Drag|null>(null),suppressClick=useRef(false),scrollFrame=useRef(0);
 const [held,setHeld]=useState<string|null>(null),[dragging,setDragging]=useState<string|null>(null);
 const [dropTarget,setDropTarget]=useState<TabDropTarget|null>(null),[preview,setPreview]=useState<TabDragPreview|null>(null);
 const targetAt=useCallback((point:Point,id:string):TabDropTarget|null=>{
  const hit=document.elementFromPoint(point.x,point.y);
  const anchorId=hit?.closest<HTMLElement>('[data-tab-group-anchor]')?.dataset.tabGroupAnchor;
  if(anchorId)return groupDropTarget(current.current.session,id,anchorId);
  const element=hit?.closest<HTMLElement>('[data-tab-id]');
  if(!element)return null;
  const target=element.dataset.tabId!,rect=element.getBoundingClientRect();
  const edge=element.dataset.tabAxis==='vertical'?(point.y>rect.top+rect.height/2?'after':'before'):(point.x>rect.left+rect.width/2?'after':'before');
  return reorderAction(current.current.session,id,target,edge)?{id:target,edge}:null;
 },[]);
 const updateFeedback=useCallback((d:Drag)=>{
  const target=targetAt(d.point,d.id);
  setDropTarget(previous=>previous?.id===target?.id&&previous?.edge===target?.edge?previous:target);
  setPreview(previous=>previous&&previous.point.x===d.point.x&&previous.point.y===d.point.y&&previous.target?.id===target?.id&&previous.target?.edge===target?.edge?previous:{id:d.id,point:{...d.point},target});
 },[targetAt]);
 const stop=useCallback(()=>{
  const previous=drag.current;drag.current=null;
  cancelAnimationFrame(scrollFrame.current);scrollFrame.current=0;
  if(previous?.capture.hasPointerCapture(previous.pointerId))previous.capture.releasePointerCapture(previous.pointerId);
  setHeld(null);setDragging(null);setDropTarget(null);setPreview(null);
  return previous;
 },[]);
 const cancelTabDrag=useCallback(()=>{if(drag.current?.active)suppressClick.current=true;stop()},[stop]);
 const scroll=useCallback(function tick(){
  const d=drag.current;if(!d?.active)return;
  const beneath=document.elementFromPoint(d.point.x,d.point.y);
  const container=beneath?.closest<HTMLElement>('.tab-scroll,.rail-scroll');
  if(container){
   const rect=container.getBoundingClientRect();
   if(container.classList.contains('tab-scroll'))container.scrollLeft+=edgeScroll(d.point.x,rect.left,rect.right);
   else container.scrollTop+=edgeScroll(d.point.y,rect.top,rect.bottom);
   updateFeedback(d);
  }
  scrollFrame.current=requestAnimationFrame(tick);
 },[updateFeedback]);
 const move=useCallback((event:PointerEvent)=>{
  const d=drag.current;if(!d||event.pointerId!==d.pointerId)return;
  if(!(event.buttons&1)){cancelTabDrag();return}
  d.point={x:event.clientX,y:event.clientY};
  if(!d.active&&Math.hypot(d.point.x-d.start.x,d.point.y-d.start.y)>7){
   d.active=true;setDragging(d.id);scrollFrame.current=requestAnimationFrame(scroll);
  }
  if(d.active){event.preventDefault();updateFeedback(d)}
 },[cancelTabDrag,scroll,updateFeedback]);
 const up=useCallback((event:PointerEvent)=>{
  if(!drag.current||event.pointerId!==drag.current.pointerId)return;
  const d=stop();if(!d?.active)return;
  suppressClick.current=true;
  const point={x:event.clientX,y:event.clientY},target=targetAt(point,d.id);
  if(target?.edge==='group'){
   void rpc('group_tab',{id:d.id,group:target.group,workspace:target.workspace}).catch(current.current.callbacks.onError);
   return;
  }
  const action=target&&reorderAction(current.current.session,d.id,target.id,target.edge);
  if(action)void rpc('reorder_tab',action).catch(current.current.callbacks.onError);
  else if(point.x<0||point.y<0||point.x>=window.innerWidth||point.y>=window.innerHeight){
   current.current.callbacks.onNotice('Flytter fanen til et nytt vindu …');
   void rpc<boolean>('detach_tab',{id:d.id,outside_only:true}).then(moved=>current.current.callbacks.onNotice(moved?'Fanen er flyttet til et nytt vindu.':'')).catch(current.current.callbacks.onError);
  }
 },[stop,targetAt]);
 const tabPointerDown=useCallback((event:ReactPointerEvent<HTMLElement>,id:string)=>{
  if(event.button!==0||event.isPrimary===false||(event.target as Element).closest('.tab-close'))return;
  const tab=current.current.session.tabs.find(tab=>tab.id===id);if(!tab)return;
  stop();suppressClick.current=false;
  const capture=(event.target as Element).closest('button')??event.currentTarget;
  drag.current={id,pointerId:event.pointerId,start:{x:event.clientX,y:event.clientY},point:{x:event.clientX,y:event.clientY},active:false,capture,workspace:tab.workspace||'Personlig',pinned:tab.pinned};
  try{capture.setPointerCapture(event.pointerId)}catch{ /* Window listeners still finish/cancel the gesture. */ }
  setHeld(id);
 },[stop]);
 const tabPointerMove=useCallback((event:ReactPointerEvent<HTMLElement>)=>move(event.nativeEvent),[move]);
 const tabPointerUp=useCallback((event:ReactPointerEvent<HTMLElement>)=>up(event.nativeEvent),[up]);
 const lostPointerCapture=useCallback((event:ReactPointerEvent<HTMLElement>)=>{
  if(drag.current?.pointerId===event.pointerId)cancelTabDrag();
 },[cancelTabDrag]);
 const suppressDragClick=useCallback((event:ReactMouseEvent<HTMLElement>)=>{
  if(suppressClick.current){event.preventDefault();event.stopPropagation();suppressClick.current=false}
 },[]);
 useEffect(()=>{
  const cancelPointer=(event:PointerEvent)=>{if(drag.current?.pointerId===event.pointerId)cancelTabDrag()};
  const key=(event:KeyboardEvent)=>{if(event.key==='Escape'&&drag.current){event.preventDefault();cancelTabDrag()}};
  const hidden=()=>{if(document.hidden)cancelTabDrag()};
  window.addEventListener('pointermove',move,{passive:false});window.addEventListener('pointerup',up);
  window.addEventListener('pointercancel',cancelPointer);window.addEventListener('blur',cancelTabDrag);
  window.addEventListener('keydown',key,true);document.addEventListener('visibilitychange',hidden);
  return()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',up);window.removeEventListener('pointercancel',cancelPointer);window.removeEventListener('blur',cancelTabDrag);window.removeEventListener('keydown',key,true);document.removeEventListener('visibilitychange',hidden);stop()};
 },[move,up,cancelTabDrag,stop]);
 useEffect(()=>{
  const d=drag.current;if(!d)return;
  const tab=session.tabs.find(tab=>tab.id===d.id);
  if(!tab||(session.workspace||'Personlig')!==d.workspace||tab.pinned!==d.pinned)cancelTabDrag();
 },[session,cancelTabDrag]);
 useEffect(()=>{
  if(held)document.documentElement.dataset.tabDrag=dragging?'dragging':'held';
  else delete document.documentElement.dataset.tabDrag;
  return()=>{delete document.documentElement.dataset.tabDrag};
 },[held,dragging]);
 return {held,dragging,dropTarget,preview,tabPointerDown,tabPointerMove,tabPointerUp,cancelTabDrag,suppressDragClick,lostPointerCapture};
}
