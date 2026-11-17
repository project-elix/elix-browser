import type {Session} from '../types';

export const splitShortcut = 'Ctrl+Alt+S';
export function isSplitShortcut(event: Pick<KeyboardEvent,'key'|'ctrlKey'|'altKey'|'shiftKey'|'metaKey'|'repeat'|'defaultPrevented'|'isComposing'|'getModifierState'>) {
 return event.ctrlKey && event.altKey && !event.shiftKey && !event.metaKey && !event.repeat &&
  !event.defaultPrevented && !event.isComposing && !event.getModifierState('AltGraph') && event.key.toLowerCase()==='s';
}
export function splitCandidates(session:Session,excludedId?:string|null) {
 const active=session.tabs.find(tab=>tab.id===session.active);
 if(!active||active.id===excludedId||active.url.startsWith('elix:'))return [];
 return session.tabs.filter(tab=>tab.id!==active.id&&tab.id!==excludedId&&!tab.url.startsWith('elix:')&&
  (tab.workspace||'Personlig')===(active.workspace||'Personlig')&&tab.private===active.private);
}
export function splitAction(session:Session,excludedId?:string|null):{other:string|null}|null {
 if(session.split)return {other:null};
 const candidates=splitCandidates(session,excludedId);
 const index=session.tabs.findIndex(tab=>tab.id===session.active);
 const next=session.tabs.slice(index+1).find(tab=>candidates.some(candidate=>candidate.id===tab.id))??candidates[0];
 return next?{other:next.id}:null;
}

const internalPages=new Set(['start','developer','settings','profiles','bookmarks','history','downloads','gaming']);
function internalUrl(value:string):string|null {
 const trimmed=value.trim();
 if(!/^elix:\/\//i.test(trimmed))return null;
 try {
  const url=new URL(trimmed),page=url.hostname.toLowerCase();
  if(url.protocol!=='elix:'||!internalPages.has(page)||url.username||url.password||url.port||
   (url.pathname!==''&&url.pathname!=='/')||url.search||url.hash)return null;
  return `elix://${page}`;
 } catch {return null;}
}

export function internalNavigation(session:Session,url:string):{action:'activate';id:string}|{action:'new_tab';url:string;private:false}|null {
 const destination=internalUrl(url);
 if(!destination)return null;
 const workspace=session.workspace||'Personlig';
 const matches=session.tabs.filter(tab=>!tab.private&&(tab.workspace||'Personlig')===workspace&&internalUrl(tab.url)===destination);
 const existing=matches.find(tab=>tab.id===session.active)??matches[0];
 return existing?{action:'activate',id:existing.id}:{action:'new_tab',url:destination,private:false};
}

export function reorderAction(session:Session,id:string,targetId:string,edge:'before'|'after'):{id:string;before:string|null}|null {
 if(id===targetId)return null;
 const source=session.tabs.find(tab=>tab.id===id),target=session.tabs.find(tab=>tab.id===targetId);
 if(!source||!target||source.pinned!==target.pinned||
  (source.workspace||'Personlig')!==(target.workspace||'Personlig'))return null;
 const partition=session.tabs.filter(tab=>tab.pinned===source.pinned&&(tab.workspace||'Personlig')===(source.workspace||'Personlig'));
 const remaining=partition.filter(tab=>tab.id!==id);
 const insertion=remaining.findIndex(tab=>tab.id===targetId)+(edge==='after'?1:0);
 const ordered=[...remaining.slice(0,insertion),source,...remaining.slice(insertion)];
 if(ordered.every((tab,index)=>tab.id===partition[index].id))return null;
 return {id,before:remaining[insertion]?.id??null};
}

export function shouldFocusNewTab(session:Session,pendingId:string|null|undefined):boolean {
 return !!pendingId&&session.active===pendingId&&session.tabs.some(tab=>tab.id===pendingId&&tab.url==='elix://start');
}
