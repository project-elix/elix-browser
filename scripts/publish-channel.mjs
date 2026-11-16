import{execFileSync}from'node:child_process';import{readFileSync,writeFileSync,mkdirSync}from'node:fs';import{makeManifest,validateVersion,isNewer,findVerifiedRelease,publishedInstallerUrl}from'./release-lib.mjs';
const repo=process.env.GITHUB_REPOSITORY,version=process.env.RELEASE_VERSION,channel=process.env.RELEASE_CHANNEL;validateVersion(version,channel);if(!/^[-\w]+\/[-\w]+$/.test(repo))throw new Error('Invalid repository');
const gh=(...args)=>execFileSync('gh',args,{encoding:'utf8'});const tag=`v${version}`;
// The tag endpoint only resolves published releases; authenticated listing includes drafts.
const releases=JSON.parse(gh('api','--paginate','--slurp',`repos/${repo}/releases?per_page=100`)).flat();
const repair=process.env.REPAIR_CHANNEL_METADATA==='true';
const release=findVerifiedRelease(releases,tag,process.env.ELIX_EXPECTED_SOURCE??process.env.GITHUB_SHA,!repair);
const exe=release.assets.filter(a=>a.name.endsWith('.exe'));if(exe.length!==1)throw new Error('Expected one NSIS installer');
const sig=release.assets.find(a=>a.name===exe[0].name+'.sig');if(!sig)throw new Error('Missing signature asset');
mkdirSync('channel-output',{recursive:true});gh('release','download',tag,'--repo',repo,'--pattern',sig.name,'--dir','channel-output','--clobber');
const signature=readFileSync(`channel-output/${sig.name}`,'utf8');
let channelsRelease=null; try{channelsRelease=JSON.parse(gh('api',`repos/${repo}/releases/tags/channels`));}catch{}
let index={}; if(channelsRelease?.body){try{index=JSON.parse(channelsRelease.body);}catch{throw new Error('Channel index is not valid JSON');}}
if(repair){if(index[channel]?.version!==version)throw new Error('Repair must keep the current channel version');}
else if(index[channel]&&!isNewer(version,index[channel].version))throw new Error('Channel versions must increase');
// Validate the signature/version before any publication mutation.
makeManifest({version,channel,url:exe[0].browser_download_url,signature});
// Repair only updates channel metadata; immutable installer/signature assets are untouched.
if(!repair)gh('release','edit',tag,'--repo',repo,'--draft=false',...(channel==='stable'?['--latest']:['--prerelease','--latest=false']));
// GitHub draft assets use a temporary untagged URL. Refresh AFTER publication.
const published=JSON.parse(gh('api',`repos/${repo}/releases/${release.id}`));
const finalUrl=publishedInstallerUrl(published,exe[0].id,repo,version);
const manifest=makeManifest({version,channel,notes:published.body??'',url:finalUrl,signature,date:published.published_at});
writeFileSync(`channel-output/${channel}.json`,JSON.stringify(manifest,null,2));
try{gh('release','view','channels','--repo',repo);}catch{gh('release','create','channels','--repo',repo,'--title','ELIX update channels','--notes','{}','--prerelease','--latest=false');}
gh('release','upload','channels',`channel-output/${channel}.json`,'--repo',repo,'--clobber');index[channel]=manifest;
const current=JSON.parse(gh('api',`repos/${repo}/releases/tags/channels`));
writeFileSync('channel-output/index-body.json',JSON.stringify({body:JSON.stringify(index,null,2)}));
gh('api','--method','PATCH',`repos/${repo}/releases/${current.id}`,'--input','channel-output/index-body.json');
console.log(`Published ${channel}: ${version}`);
