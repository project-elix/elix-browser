export function validateVersion(version,channel){
 const match=/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(beta|dev)\.(0|[1-9]\d*))?$/.exec(version);
 if(!match)throw new Error('Use MAJOR.MINOR.PATCH, -beta.N or -dev.N');
 if(!['stable','beta','developer'].includes(channel))throw new Error('Unknown channel');
 if(channel==='stable'&&match[4]||channel==='beta'&&match[4]!=='beta'||channel==='developer'&&match[4]!=='dev')throw new Error('Version does not match channel');return version;
}
export function makeManifest({version,channel,notes,url,signature,date}){
 validateVersion(version,channel);const u=new URL(url);if(u.protocol!=='https:')throw new Error('HTTPS required');if(!signature?.trim())throw new Error('Signature required');
 const decoded=Buffer.from(signature.trim(),'base64').toString('utf8');if(!decoded.includes(`version:${version}`))throw new Error('Signature must bind this release version');
 return {version,notes,pub_date:date,platforms:{'windows-x86_64':{url:u.href,signature:signature.trim()}}};
}
export function isNewer(a,b){const parts=v=>{const m=/^(\d+)\.(\d+)\.(\d+)(?:-(beta|dev)\.(\d+))?$/.exec(v);if(!m)throw new Error('Invalid previous channel version');return [Number(m[1]),Number(m[2]),Number(m[3]),m[4]?0:1,m[4]??'',Number(m[5]??0)];};const x=parts(a),y=parts(b);for(let i=0;i<x.length;i++){if(x[i]!==y[i])return x[i]>y[i]}return false;}
