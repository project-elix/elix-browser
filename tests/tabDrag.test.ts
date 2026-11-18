import {describe,it,expect} from 'vitest';
import {dragPreviewPosition,edgeScroll,groupDropTarget} from '../src/browser/tabDrag';
import type {Session,Tab} from '../src/types';

describe('Dragging to a sidebar group heading',()=>{
 const tab:Tab={id:'source',url:'elix://start',title:'Start',pinned:false,private:false,loading:false,
  suspended:false,last_active:0,load_ms:null,http_status:null,navigation_error:null};
 const session:Session={active:tab.id,tabs:[
  tab,{...tab,id:'target',group:'Spill',workspace:'Personlig'},
  {...tab,id:'pinned',pinned:true,group:'Festet'},
  {...tab,id:'foreign',workspace:'Jobb',group:'Arbeid'},
 ]};
 it('offers the named group in the same workspace and pinned partition',()=>{
  expect(groupDropTarget(session,'source','target')).toEqual({id:'target',edge:'group',group:'Spill',workspace:'Personlig'});
 });
 it('rejects missing groups, redundant moves and incompatible tabs',()=>{
  for(const [source,anchor] of [['source','missing'],['target','source'],['target','target'],['source','pinned'],['source','foreign']]){
   expect(groupDropTarget(session,source,anchor)).toBeNull();
  }
 });
});

describe('Tab drag shell geometry',()=>{
 it('only scrolls inside the edge zone and increases speed towards the edge',()=>{
  expect(edgeScroll(-1,0,100)).toBe(0);expect(edgeScroll(101,0,100)).toBe(0);
  expect(edgeScroll(50,0,100)).toBe(0);
  expect(edgeScroll(1,0,100)).toBeLessThan(edgeScroll(20,0,100));
  expect(edgeScroll(99,0,100)).toBeGreaterThan(edgeScroll(80,0,100));
  expect(edgeScroll(2,0,0)).toBe(0);
 });
 it('keeps the preview on screen for internal pages and small windows',()=>{
  const placement=dragPreviewPosition({x:1400,y:900},1440,950);
  expect(placement.left+placement.width).toBeLessThanOrEqual(1432);
  expect(placement.top+54).toBeLessThanOrEqual(942);
  expect(placement.docked).toBe(false);
  const small=dragPreviewPosition({x:100,y:100},200,160);
  expect(small.left+small.width).toBeLessThanOrEqual(192);
 });
 it('keeps sidebar ghosts outside the native child view',()=>{
  const native={left:240,top:86,right:1440,bottom:920};
  const placement=dragPreviewPosition({x:220,y:400},1440,950,native);
  expect(placement.left+placement.width).toBeLessThanOrEqual(native.left-8);
  expect(placement.top).toBe(414);
  expect(placement.docked).toBe(false);
 });
 it('docks above the native view when dragging over the page',()=>{
  const native={left:240,top:86,right:1440,bottom:920};
  const placement=dragPreviewPosition({x:800,y:500},1440,950,native);
  expect(placement.top+54).toBeLessThanOrEqual(native.top);
  expect(placement.docked).toBe(true);
 });
 it('uses available right sidepanel space without covering native content',()=>{
  const native={left:240,top:86,right:1140,bottom:920};
  const placement=dragPreviewPosition({x:1150,y:500},1440,950,native);
  expect(placement.left).toBeGreaterThanOrEqual(native.right+8);
  expect(placement.docked).toBe(false);
 });
});
