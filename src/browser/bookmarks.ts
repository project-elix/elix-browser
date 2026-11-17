import type {Entry} from '../types';
export interface BookmarkGroup {name:string;items:Entry[]}
export function bookmarkGroups(items:Entry[],names:string[],filter=''):BookmarkGroup[]{
 const needle=filter.trim().toLocaleLowerCase('nb-NO');
 const groups=new Map<string,Entry[]>([['',[]]]);
 for(const name of names)if(name)groups.set(name,[]);
 for(const item of items){const name=item.group||'';if(!groups.has(name))groups.set(name,[]);if(!needle||`${item.title} ${item.url} ${name}`.toLocaleLowerCase('nb-NO').includes(needle))groups.get(name)!.push(item)}
 return [...groups].filter(([name,entries])=>!needle||entries.length||name.toLocaleLowerCase('nb-NO').includes(needle)).map(([name,items])=>({name,items}));
}
export function bookmarkAddress(address:string):string{
 const url=new URL(address.trim());
 if(!['http:','https:'].includes(url.protocol)||url.username||url.password||url.href.length>8192)throw new Error('Bruk en HTTP- eller HTTPS-adresse uten brukernavn eller passord.');
 return url.href;
}
