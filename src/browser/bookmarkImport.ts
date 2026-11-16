import type {Entry} from '../types';
export function importLinks(links:{url:string;title:string}[]):Entry[]{
 const seen=new Set<string>();const out:Entry[]=[];
 for(const item of links){try{const url=new URL(item.url);if(!['https:','http:'].includes(url.protocol)||url.username||url.password||url.href.length>8192||seen.has(url.href))continue;seen.add(url.href);out.push({id:0,url:url.href,title:(item.title||url.hostname).slice(0,250),time:'',favicon:null});}catch{/* Unsupported bookmarklets and local files are omitted. */}}
 if(out.length>5000)throw new Error('Importer maksimalt 5000 bokmerker om gangen.');return out;
}
export function parseBookmarkFile(html:string):Entry[]{
 if(html.length>10_000_000)throw new Error('Bokmerkefilen er for stor (maks. 10 MB).');
 const doc=new DOMParser().parseFromString(html,'text/html');
 return importLinks([...doc.querySelectorAll('a[href]')].map(a=>({url:a.getAttribute('href')??'',title:a.textContent??''})));
}
