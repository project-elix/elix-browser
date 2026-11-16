import {describe,it,expect} from 'vitest';
import {importLinks} from '../src/browser/bookmarkImport';
import {encrypt,decrypt} from '../src/profiles/sync';
import {defaultAppearance} from '../src/themes/theme';
describe('Alpha 6 bookmark import',()=>{
 it('normalizes URLs, keeps unique links and drops executable/local URLs',()=>{const items=importLinks([{url:'https://example.com',title:'Example'},{url:'https://example.com/',title:'Duplicate'},{url:'javascript:alert(1)',title:'Unsafe'},{url:'file:///passwords',title:'Local'},{url:'https://user:pass@example.com',title:'Secret'},{url:'http://localhost:3000/a',title:'Dev'}]);expect(items.map(i=>i.url)).toEqual(['https://example.com/','http://localhost:3000/a']);});
});
describe('Opt-in encrypted sync',()=>{
 const data={version:1 as const,items:importLinks([{url:'https://private.example/bookmark',title:'Saved title'}]),appearance:defaultAppearance};
 it('round-trips with fresh salt/nonce and reveals no bookmark text',async()=>{const a=await encrypt(data,'a sufficiently long phrase'),b=await encrypt(data,'a sufficiently long phrase');expect(a.salt).not.toBe(b.salt);expect(a.iv).not.toBe(b.iv);expect(JSON.stringify(a)).not.toContain('private.example');expect(await decrypt(a,'a sufficiently long phrase')).toEqual(data);});
 it('rejects the wrong phrase, tampering and unknown parameters',async()=>{const a=await encrypt(data,'a sufficiently long phrase');await expect(decrypt(a,'another sufficiently long phrase')).rejects.toThrow();await expect(decrypt({...a,iv:btoa('abcdefghijkl')},'a sufficiently long phrase')).rejects.toThrow();await expect(decrypt({...a,iterations:1} as never,'a sufficiently long phrase')).rejects.toThrow();});
});
