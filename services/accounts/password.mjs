import {randomBytes,scrypt,timingSafeEqual} from 'node:crypto';
import {promisify} from 'node:util';

const derive=promisify(scrypt);
const options={N:131072,r:8,p:1,maxmem:160*1024*1024};
export function validatePassword(value){
  if(typeof value!=='string'||[...value].length<15||Buffer.byteLength(value)>512){
    throw Object.assign(new Error('Bruk et passord med minst 15 tegn og maks 512 byte.'),{status:400});
  }
}
export function passwordStore(){
  // Bound native allocations across simultaneous HTTP requests.
  let active=0;
  const stretch=async(password,salt)=>{
    if(active>=2)throw Object.assign(new Error('Tjenesten er opptatt. Prøv igjen.'),{status:503});
    active++;
    try{return await derive(password,salt,32,options);}finally{active--}
  };
  const dummySalt=randomBytes(16),dummyDigest=randomBytes(32);
  return {
    async create(password){
      const salt=randomBytes(16);
      return {salt,digest:await stretch(password,salt)};
    },
    async matches(password,record){
      const actual=await stretch(password,record?.salt??dummySalt);
      return timingSafeEqual(actual,record?.digest??dummyDigest)&&!!record;
    },
  };
}
