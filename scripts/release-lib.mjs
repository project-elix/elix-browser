export function validateVersion(version,channel){
 const match=/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(beta|dev)\.(0|[1-9]\d*))?$/.exec(version);
 if(!match)throw new Error('Use MAJOR.MINOR.PATCH, -beta.N or -dev.N');
 if(!['stable','beta','developer'].includes(channel))throw new Error('Unknown channel');
 if(channel==='stable'&&match[4]||channel==='beta'&&match[4]!=='beta'||channel==='developer'&&match[4]!=='dev')throw new Error('Version does not match channel');return version;
}
export function makeManifest({version,channel,notes,url,signature,date}){
 validateVersion(version,channel);const u=new URL(url);if(u.protocol!=='https:')throw new Error('HTTPS required');if(!signature?.trim())throw new Error('Signature required');
 const decoded=Buffer.from(signature.trim(),'base64').toString('utf8');if(decoded.match(/(?:^|[\t ])version:([^\s]+)/m)?.[1]!==version)throw new Error('Signature must bind this release version');
 return {version,notes,pub_date:date,platforms:{'windows-x86_64':{url:u.href,signature:signature.trim()}}};
}
export function isNewer(a,b){const parts=v=>{const m=/^(\d+)\.(\d+)\.(\d+)(?:-(beta|dev)\.(\d+))?$/.exec(v);if(!m)throw new Error('Invalid previous channel version');return [Number(m[1]),Number(m[2]),Number(m[3]),m[4]?0:1,m[4]??'',Number(m[5]??0)];};const x=parts(a),y=parts(b);for(let i=0;i<x.length;i++){if(x[i]!==y[i])return x[i]>y[i]}return false;}

export function findVerifiedRelease(releases,tag,source,draft=true){
 const matches=releases.filter(r=>r.tag_name===tag);
 if(matches.length!==1)throw new Error('Expected exactly one release draft');
 const release=matches[0];
 if(release.draft!==draft)throw new Error('Release has an unexpected publication state');
 if(!/^[a-f0-9]{40}$/.test(source??'')||release.target_commitish!==source)throw new Error('Draft does not match verified source');
 return release;
}
export function verifiedReleaseSource(run,jobs){
 if(run.path!=='.github/workflows/release.yml'||run.event!=='workflow_dispatch'||run.head_branch!=='main'||run.status!=='completed'||!/^[a-f0-9]{40}$/.test(run.head_sha??''))throw new Error('Expected completed main release workflow');
 for(const name of ['native-verification / windows','upgrade-verification / upgrade','build']){
  const matches=jobs.filter(j=>j.name===name);
  if(matches.length!==1||matches[0].run_id!==run.id||matches[0].conclusion!=='success')throw new Error(`Required release gate did not pass: ${name}`);
 }
 return run.head_sha;
}

export const findDraft=(releases,tag,source)=>findVerifiedRelease(releases,tag,source,true);
export function publishedInstallerUrl(release,assetId,repo,version){
 if(release.draft||release.tag_name!==`v${version}`)throw new Error('Release is not published at the expected tag');
 const asset=release.assets.find(a=>a.id===assetId);
 if(!asset)throw new Error('Original installer asset missing');
 const url=new URL(asset.browser_download_url);
 if(url.protocol!=='https:'||url.hostname!=='github.com'||!url.pathname.startsWith(`/${repo}/releases/download/v${version}/`)||!url.pathname.endsWith('.exe'))throw new Error('Expected final immutable installer URL');
 return url.href;
}
