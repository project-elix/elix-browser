import {describe,it,expect} from 'vitest';
import {internalNavigation,isSplitShortcut,reorderAction,shouldFocusNewTab,splitAction,splitCandidates} from '../src/browser/tabTools';
import type {Session,Tab} from '../src/types';

const tab=(id:string,overrides:Partial<Tab>={}):Tab=>({id,url:`https://${id}.example`,title:id,loading:false,suspended:false,last_active:0,load_ms:null,http_status:null,navigation_error:null,private:false,pinned:false,...overrides});
const key=(overrides:Partial<Parameters<typeof isSplitShortcut>[0]>={})=>({key:'s',ctrlKey:true,altKey:true,shiftKey:false,metaKey:false,repeat:false,defaultPrevented:false,isComposing:false,getModifierState:()=>false,...overrides});

describe('Split view shortcut',()=>{
 it('matches Ctrl+Alt+S without consuming other shortcuts or AltGraph input',()=>{
  expect(isSplitShortcut(key())).toBe(true);
  expect(isSplitShortcut(key({key:'S'}))).toBe(true);
  for(const changes of [{ctrlKey:false},{altKey:false},{shiftKey:true},{metaKey:true},{key:'a'},{getModifierState:(name:string)=>name==='AltGraph'}])expect(isSplitShortcut(key(changes))).toBe(false);
 });
 it('ignores held keys, composed text and already handled keyboard events',()=>{
  for(const changes of [{repeat:true},{isComposing:true},{defaultPrevented:true}])expect(isSplitShortcut(key(changes))).toBe(false);
 });
 it('pairs the active page with the next compatible page and wraps at the end',()=>{
  const session:Session={tabs:[tab('one'),tab('two'),tab('three')],active:'two'};
  expect(splitAction(session)).toEqual({other:'three'});
  expect(splitAction({...session,active:'three'})).toEqual({other:'one'});
 });
 it('never mixes workspaces, private sessions or internal pages',()=>{
  const session:Session={active:'active',tabs:[tab('active',{workspace:'Jobb'}),tab('private',{workspace:'Jobb',private:true}),tab('other',{workspace:'Personlig'}),tab('settings',{workspace:'Jobb',url:'elix://settings'}),tab('matching',{workspace:'Jobb'})]};
  expect(splitCandidates(session).map(t=>t.id)).toEqual(['matching']);
  expect(splitAction(session)).toEqual({other:'matching'});
  expect(splitAction({...session,active:'settings'})).toBeNull();
  expect(splitAction({...session,tabs:session.tabs.slice(0,4)})).toBeNull();
 });
 it('handles older tabs without an explicit workspace and exits an existing split',()=>{
  const session:Session={tabs:[tab('one'),tab('two',{workspace:'Personlig'})],active:'one'};
  expect(splitAction(session)).toEqual({other:'two'});
  expect(splitAction({...session,split:['one','two']})).toEqual({other:null});
 });
});

describe('Internal navigation',()=>{
 it('reuses the current ordinary internal tab without replacing a page or switching workspaces',()=>{
  const session:Session={active:'current',workspace:'Jobb',tabs:[
   tab('other-workspace',{workspace:'Personlig',url:'elix://settings'}),
   tab('private',{workspace:'Jobb',private:true,url:'elix://settings'}),
   tab('duplicate',{workspace:'Jobb',url:'elix://settings'}),
   tab('current',{workspace:'Jobb',url:'elix://settings'}),
  ]};
  expect(internalNavigation(session,'elix://settings')).toEqual({action:'activate',id:'current'});
  expect(internalNavigation({...session,active:'private'},'elix://settings')).toEqual({action:'activate',id:'duplicate'});
 });
 it('opens an ordinary internal tab when only private tabs or other workspaces match',()=>{
  const session:Session={active:'private',workspace:'Jobb',tabs:[
   tab('private',{workspace:'Jobb',private:true,url:'elix://history'}),
   tab('other-workspace',{workspace:'Personlig',url:'elix://history'}),
  ]};
  expect(internalNavigation(session,'elix://history')).toEqual({action:'new_tab',url:'elix://history',private:false});
 });
 it('normalizes canonical internal destinations and legacy default-workspace tabs',()=>{
  const session:Session={active:'page',tabs:[tab('page'),tab('settings',{url:'elix://settings'})]};
  expect(internalNavigation(session,'  ELIX://SETTINGS/  ')).toEqual({action:'activate',id:'settings'});
  for(const page of ['start','developer','profiles','bookmarks','history','downloads','gaming']) {
   expect(internalNavigation(session,` elix://${page.toUpperCase()}/ `)).toEqual({action:'new_tab',url:`elix://${page}`,private:false});
  }
 });
 it('leaves external URLs, search text and noncanonical internal destinations to the caller',()=>{
  const session:Session={active:'page',tabs:[tab('page')]};
  for(const url of ['https://example.com','example.com','settings','elix:settings','elix://unknown','elix://settings/profile','elix://settings?query=1','elix://settings#section','elix://user@settings','elix://settings:3000','elix://']) {
   expect(internalNavigation(session,url),url).toBeNull();
  }
 });
});

describe('Tab reordering',()=>{
 const session:Session={active:'one',tabs:[tab('one'),tab('two'),tab('three'),tab('four')]};
 it('computes forward and backward drops against the list after removing the source',()=>{
  expect(reorderAction(session,'one','three','before')).toEqual({id:'one',before:'three'});
  expect(reorderAction(session,'one','three','after')).toEqual({id:'one',before:'four'});
  expect(reorderAction(session,'four','two','before')).toEqual({id:'four',before:'two'});
  expect(reorderAction(session,'four','two','after')).toEqual({id:'four',before:'three'});
 });
 it('appends after the final eligible tab and suppresses already-correct positions',()=>{
  expect(reorderAction(session,'one','four','after')).toEqual({id:'one',before:null});
  expect(reorderAction(session,'one','two','before')).toBeNull();
  expect(reorderAction(session,'two','one','after')).toBeNull();
  expect(reorderAction(session,'four','three','after')).toBeNull();
  expect(reorderAction(session,'two','two','before')).toBeNull();
 });
 it('rejects unknown tabs and crossings between workspaces or pinned partitions',()=>{
  const mixed:Session={active:'one',tabs:[tab('one'),tab('pinned',{pinned:true}),tab('work',{workspace:'Jobb'})]};
  for(const [source,target] of [['one','missing'],['missing','one'],['one','pinned'],['pinned','one'],['one','work'],['work','one']]) {
   expect(reorderAction(mixed,source,target,'before')).toBeNull();
   expect(reorderAction(mixed,source,target,'after')).toBeNull();
  }
 });
 it('skips interleaved foreign tabs and preserves tab groups and all session data',()=>{
  const mixed:Session={active:'one',tabs:[
   tab('one',{group:'Lesing'}),tab('pinned',{pinned:true}),tab('two',{workspace:'Personlig',group:'Gaming'}),
   tab('work',{workspace:'Jobb'}),tab('three',{private:true,group:'Privat'}),tab('last-work',{workspace:'Jobb'}),
  ]};
  const original=structuredClone(mixed);
  expect(reorderAction(mixed,'one','two','after')).toEqual({id:'one',before:'three'});
  expect(reorderAction(mixed,'one','three','after')).toEqual({id:'one',before:null});
  expect(reorderAction(mixed,'one','two','before')).toBeNull();
  expect(mixed).toEqual(original);
 });
 it('supports reordering inside the pinned partition',()=>{
  const mixed:Session={active:'one',tabs:[tab('one',{pinned:true}),tab('ordinary'),tab('two',{pinned:true}),tab('three',{pinned:true})]};
  expect(reorderAction(mixed,'three','one','before')).toEqual({id:'three',before:'one'});
  expect(reorderAction(mixed,'one','three','after')).toEqual({id:'one',before:null});
 });
});

describe('New tab address focus',()=>{
 it('focuses only the pending start tab when it becomes active',()=>{
  const session:Session={active:'created',tabs:[tab('old'),tab('created',{url:'elix://start'})]};
  expect(shouldFocusNewTab(session,'created')).toBe(true);
  expect(shouldFocusNewTab({...session,active:'old'},'created')).toBe(false);
  expect(shouldFocusNewTab({...session,tabs:[tab('old')]},'created')).toBe(false);
  expect(shouldFocusNewTab(session,null)).toBe(false);
  expect(shouldFocusNewTab(session,undefined)).toBe(false);
 });
 it('does not steal focus for an older creation event or a tab that already navigated',()=>{
  const session:Session={active:'newer',tabs:[tab('older',{url:'elix://start'}),tab('newer',{url:'elix://start'})]};
  expect(shouldFocusNewTab(session,'older')).toBe(false);
  expect(shouldFocusNewTab(session,'newer')).toBe(true);
  expect(shouldFocusNewTab({...session,tabs:[tab('newer')]},'newer')).toBe(false);
  expect(shouldFocusNewTab({...session,tabs:[tab('newer',{url:'elix://settings'})]},'newer')).toBe(false);
 });
});


describe('Split view with an active miniplayer',()=>{
 it('skips the miniplayer and selects the next eligible webpage',()=>{
  const session:Session={active:'one',tabs:[tab('one'),tab('player'),tab('three')]};
  expect(splitCandidates(session,'player').map(t=>t.id)).toEqual(['three']);
  expect(splitAction(session,'player')).toEqual({other:'three'});
 });
 it('does not open a split from the popped-out page or when it is the only peer',()=>{
  const session:Session={active:'one',tabs:[tab('one'),tab('player')]};
  expect(splitAction(session,'player')).toBeNull();
  expect(splitCandidates({...session,active:'player'},'player')).toEqual([]);
  expect(splitAction({...session,active:'player'},'player')).toBeNull();
 });
 it('can still leave an existing split and restores peers when the player returns',()=>{
  const session:Session={active:'one',tabs:[tab('one'),tab('player')]};
  expect(splitAction({...session,split:['one','player']},'player')).toEqual({other:null});
  expect(splitAction(session,null)).toEqual({other:'player'});
 });
});
