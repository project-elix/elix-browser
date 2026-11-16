import type {Entry} from '../types';

export interface Suggestion {key:string;title:string;url:string;detail:string;kind:'history'|'bookmark'|'site'|'search'}
export const popularSites=[
 {title:'Project ELIX',url:'https://projectelix.no/'},
 {title:'YouTube',url:'https://www.youtube.com/'},
 {title:'Discord',url:'https://discord.com/app'},
 {title:'Twitch',url:'https://www.twitch.tv/'},
 {title:'GitHub',url:'https://github.com/'},
 {title:'Steam',url:'https://store.steampowered.com/'}
];
export function safeWebUrl(value:string):URL|null {try{const url=new URL(value);return ['https:','http:'].includes(url.protocol)&&!url.username&&!url.password?url:null}catch{return null}}
export function hostname(value:string){return safeWebUrl(value)?.hostname??value}
function identity(url:URL){const canonical=new URL(url);canonical.hash='';return canonical.href}
function searchTerm(url:URL){return /(^|\.)google\.[a-z.]+$/.test(url.hostname)&&url.pathname==='/search'?(url.searchParams.get('q')??''):''}

/** Suggestions stay local. Typing never sends the query or history to a server. */
export function addressSuggestions(query:string,history:Entry[],bookmarks:Entry[],limit=6):Suggestion[]{
 const q=query.trim().toLocaleLowerCase('nb-NO'),tokens=q.split(/\s+/).filter(Boolean);
 const candidates=new Map<string,{item:Suggestion;visits:number;recent:number;bookmark:boolean}>();
 for(const [entries,bookmark] of [[history,false],[bookmarks,true]] as const){
  for(const entry of entries){
   const url=safeWebUrl(entry.url);if(!url)continue;
   const key=identity(url),old=candidates.get(key),term=searchTerm(url);
   const title=term||entry.title.trim()||url.hostname;
   const recent=Date.parse(entry.time)||0;
   candidates.set(key,{item:{key,title,url:entry.url,detail:bookmark?'Bokmerke':term?'Tidligere søk':url.hostname,kind:bookmark?'bookmark':'history'},visits:(old?.visits??0)+(bookmark?0:1),recent:Math.max(old?.recent??0,recent),bookmark:bookmark||!!old?.bookmark});
  }
 }
 const ranked=[...candidates.values()].filter(({item})=>tokens.every(token=>`${item.title} ${item.url}`.toLocaleLowerCase('nb-NO').includes(token))).sort((a,b)=>{
  const score=(v:typeof a)=>Math.min(v.visits,40)*2+(v.bookmark?8:0)+(q&&(hostname(v.item.url).replace(/^www\./,'').startsWith(q)||v.item.title.toLocaleLowerCase('nb-NO').startsWith(q))?50:0);
  return score(b)-score(a)||b.recent-a.recent;
 });
 const result=ranked.slice(0,limit).map(v=>({...v.item,kind:v.bookmark?'bookmark' as const:v.item.kind,detail:v.bookmark?'Bokmerke':v.item.detail}));
 // Known sites are fallback suggestions, never presented as the user's history.
 if(result.length<limit){for(const site of popularSites){if(result.length===limit)break;const key=identity(new URL(site.url));if(!candidates.has(key)&&tokens.every(token=>`${site.title} ${site.url}`.toLocaleLowerCase('nb-NO').includes(token)))result.push({key,...site,detail:'Nettsted',kind:'site'})}}
 if(q){result.splice(Math.max(0,limit-1));result.push({key:'search-query',title:query.trim(),url:query.trim(),detail:'Søk på nettet',kind:'search'})}
 return result;
}
