import {invoke,isTauri} from '@tauri-apps/api/core';
export const desktop=isTauri();
export async function rpc<T=void>(action:string,data:Record<string,unknown>={}):Promise<T>{if(!desktop)throw new Error('Dette er en visning av grensesnittet. Kjør ELIX som Windows-program for å bruke nettleseren.');return invoke<T>('dispatch',{request:{action,...data}})}
export const internalTitle=(url:string)=>({'elix://start':'Ny fane','elix://developer':'Utvikler','elix://settings':'Innstillinger','elix://bookmarks':'Bokmerker','elix://history':'Historikk','elix://downloads':'Nedlastinger','elix://gaming':'Gaming'}[url]??'ELIX');
export const bytes=(n:number)=>`${(n/1024/1024).toFixed(0)} MB`;
export function progress(received:number,total?:number|null){return total&&total>0?Math.min(100,Math.max(0,received/total*100)):null}
