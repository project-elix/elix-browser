import {splitCandidates} from './tabTools';
import type {Session} from '../types';

export type Point={x:number;y:number};
export type Rect={left:number;top:number;right:number;bottom:number};

export function groupDropTarget(session:Session,sourceId:string,anchorId:string) {
 const source=session.tabs.find(tab=>tab.id===sourceId);
 const anchor=session.tabs.find(tab=>tab.id===anchorId);
 if(!source||!anchor?.group||source.group===anchor.group||source.pinned!==anchor.pinned||
   (source.workspace||'Personlig')!==(anchor.workspace||'Personlig'))return null;
 return {id:anchorId,edge:'group' as const,group:anchor.group,workspace:anchor.workspace||'Personlig'};
}

export function splitDropTarget(session:Session,sourceId:string,playerId?:string|null) {
 const candidates=splitCandidates(session,playerId);
 const peer=sourceId===session.active
  ?candidates.find(tab=>!session.split?.includes(tab.id))
  :candidates.find(tab=>tab.id===sourceId);
 if(!peer||session.split?.includes(peer.id))return null;
 return {id:peer.id,edge:'split' as const};
}

/** Slow down near the edge instead of jumping a whole tab at a time. */
export function edgeScroll(position:number,start:number,end:number,margin=28):number {
 if(position<start||position>end||end<=start)return 0;
 const zone=Math.min(margin,(end-start)/3);
 if(position<start+zone)return -Math.ceil(10*(start+zone-position)/zone);
 if(position>end-zone)return Math.ceil(10*(position-end+zone)/zone);
 return 0;
}

const clamp=(n:number,min:number,max:number)=>Math.max(min,Math.min(n,Math.max(min,max)));
/** Native child views cover DOM overlays. Keep the held tab on a visible shell surface. */
export function dragPreviewPosition(point:Point,width:number,height:number,native?:Rect|null) {
 const margin=8,previewHeight=54;
 let previewWidth=Math.min(244,Math.max(48,width-margin*2));
 let left=clamp(point.x+14,margin,width-previewWidth-margin);
 let top=clamp(point.y+14,margin,height-previewHeight-margin);
 let docked=false;
 if(native&&left<native.right&&left+previewWidth>native.left&&top<native.bottom&&top+previewHeight>native.top) {
  if(point.x<native.left&&native.left>=72) {
   previewWidth=Math.min(previewWidth,native.left-margin*2);
   left=clamp(point.x+14,margin,native.left-previewWidth-margin);
  } else if(point.x>=native.right&&width-native.right>=72) {
   previewWidth=Math.min(previewWidth,width-native.right-margin*2);
   left=clamp(point.x+14,native.right+margin,width-previewWidth-margin);
  } else {
   // The page itself owns this part of the window; keep feedback just above it.
   top=Math.max(0,native.top-previewHeight-margin);
   docked=true;
  }
 }
 return {left,top,width:previewWidth,docked};
}
