import{describe,it,expect}from'vitest';
import{addressSuggestions,safeWebUrl}from'../src/browser/suggestions';
import type{Entry}from'../src/types';
const entry=(id:number,url:string,title:string,time='2026-11-16T12:00:00Z'):Entry=>({id,url,title,time});
describe('local address suggestions',()=>{
 it('ranks frequently visited sites and deduplicates URL fragments',()=>{
  const history=[entry(1,'https://example.com/','Example'),entry(2,'https://example.com/#one','Example'),entry(3,'https://recent.test/','Recent','2026-11-16T13:00:00Z')];
  const rows=addressSuggestions('',history,[]);expect(rows[0].url).toBe('https://example.com/#one');expect(rows.filter(r=>r.url.startsWith('https://example.com/'))).toHaveLength(1);
 });
 it('finds previous searches, matching all words regardless of case',()=>{
  const rows=addressSuggestions('RUST browser',[entry(1,'https://www.google.com/search?q=rust+browser+tauri','Search')],[]);
  expect(rows[0]).toMatchObject({title:'rust browser tauri',kind:'history',detail:'Tidligere søk'});expect(rows.at(-1)).toMatchObject({kind:'search',url:'RUST browser'});
 });
 it('keeps bookmark metadata when also visited',()=>{
  const row=addressSuggestions('example',[entry(1,'https://example.com/','Example')],[entry(2,'https://example.com/','Saved example')])[0];expect(row.kind).toBe('bookmark');expect(row.title).toBe('Saved example');
 });
 it('rejects script, internal and credential URLs',()=>{
  for(const url of ['javascript:alert(1)','elix://settings','https://name:password@example.com/','not a url'])expect(safeWebUrl(url)).toBeNull();
  const rows=addressSuggestions('',[entry(1,'javascript:alert(1)','Bad')],[]);expect(rows.some(r=>r.title==='Bad')).toBe(false);
 });
 it('labels fallback sites honestly and limits the list',()=>{
  const rows=addressSuggestions('',[],[],3);expect(rows).toHaveLength(3);expect(rows.every(r=>r.kind==='site')).toBe(true);expect(addressSuggestions('no-match-123',[],[])).toEqual([{key:'search-query',title:'no-match-123',url:'no-match-123',detail:'Søk på nettet',kind:'search'}]);
 });
});
