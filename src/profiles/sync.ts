import type {Appearance,Entry} from '../types';
export interface SyncData{version:1;items:Entry[];appearance:Appearance}
export interface Envelope{version:1;algorithm:'AES-GCM';iterations:600000;salt:string;iv:string;ciphertext:string}
const encode=(bytes:Uint8Array)=>{let s='';for(let i=0;i<bytes.length;i+=8192)s+=String.fromCharCode(...bytes.subarray(i,i+8192));return btoa(s)};
const decode=(s:string,max:number)=>{if(typeof s!=='string'||s.length>max*1.4+4||!/^[A-Za-z0-9+/]+={0,2}$/.test(s))throw new Error('Ugyldig kryptert innhold');return Uint8Array.from(atob(s),c=>c.charCodeAt(0))};
async function key(password:string,salt:Uint8Array<ArrayBuffer>){if(password.length<12||password.length>1024)throw new Error('Bruk en synkroniseringsfrase på minst 12 tegn.');const material=await crypto.subtle.importKey('raw',new TextEncoder().encode(password),'PBKDF2',false,['deriveKey']);return crypto.subtle.deriveKey({name:'PBKDF2',hash:'SHA-256',salt,iterations:600000},material,{name:'AES-GCM',length:256},false,['encrypt','decrypt'])}
export async function encrypt(data:SyncData,password:string):Promise<Envelope>{const salt=crypto.getRandomValues(new Uint8Array(16)),iv=crypto.getRandomValues(new Uint8Array(12));const plaintext=new TextEncoder().encode(JSON.stringify(data));if(plaintext.length>3_900_000)throw new Error('For mye data å synkronisere');const cipher=await crypto.subtle.encrypt({name:'AES-GCM',iv,additionalData:new TextEncoder().encode('ELIX-sync-v1')},await key(password,salt),plaintext);return {version:1,algorithm:'AES-GCM',iterations:600000,salt:encode(salt),iv:encode(iv),ciphertext:encode(new Uint8Array(cipher))}}
export async function decrypt(envelope:Envelope,password:string):Promise<SyncData>{
 if(envelope.version!==1||envelope.algorithm!=='AES-GCM'||envelope.iterations!==600000)throw new Error('Ukjent synkroniseringsformat');
 const salt=decode(envelope.salt,16),iv=decode(envelope.iv,12);if(salt.length!==16||iv.length!==12)throw new Error('Ugyldig kryptering');
 let plain:ArrayBuffer;try{plain=await crypto.subtle.decrypt({name:'AES-GCM',iv,additionalData:new TextEncoder().encode('ELIX-sync-v1')},await key(password,salt),decode(envelope.ciphertext,4_000_000))}catch{throw new Error('Feil synkroniseringsfrase eller skadet innhold. Ingen lokale data er endret.')}
 const data=JSON.parse(new TextDecoder().decode(plain));if(data.version!==1||!Array.isArray(data.items)||data.items.length>5000||!data.appearance)throw new Error('Ugyldige synkroniseringsdata');return data;
}
