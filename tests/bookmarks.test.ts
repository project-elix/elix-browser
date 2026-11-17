import {describe,it,expect} from 'vitest';
import {bookmarkAddress,bookmarkGroups} from '../src/browser/bookmarks';
import type {Entry} from '../src/types';
const item=(id:number,title:string,group?:string):Entry=>({id,title,group,url:`https://example.com/${id}`,time:''});
describe('bookmark collections',()=>{
 it('keeps old ungrouped bookmarks, empty groups and grouped members',()=>{
  expect(bookmarkGroups([item(1,'Old'),item(2,'ELIX','Gaming')],['Gaming','Empty'])).toEqual([
   {name:'',items:[item(1,'Old')]},{name:'Gaming',items:[item(2,'ELIX','Gaming')]},{name:'Empty',items:[]}
  ]);
 });
 it('finds group names as well as bookmark title and URL',()=>{
  expect(bookmarkGroups([item(1,'Server','Gaming'),item(2,'Manual','Jobb')],['Gaming','Jobb'],'gaming').map(g=>g.items.map(i=>i.id))).toEqual([[1]]);
  expect(bookmarkGroups([item(1,'ELIX'),item(2,'Manual')],[],'manual')[0].items[0].id).toBe(2);
  expect(bookmarkGroups([item(1,'ELIX')],['Empty'],'missing')).toEqual([]);
 });
 it('retains groups supplied on entries during a refresh',()=>{
  expect(bookmarkGroups([item(1,'ELIX','New')],[]).at(-1)?.name).toBe('New');
 });
 it('normalizes edited URLs and rejects unsafe or credential URLs',()=>{
  expect(bookmarkAddress(' https://example.com ')).toBe('https://example.com/');
  for(const url of ['javascript:alert(1)','file:///test','https://name:secret@example.com','invalid'])expect(()=>bookmarkAddress(url)).toThrow();
 });
});
