import{execFileSync}from'node:child_process';import{writeFileSync}from'node:fs';
const channel=process.env.CHANNEL,repo=process.env.GITHUB_REPOSITORY;if(!['stable','beta','developer'].includes(channel))throw new Error('Invalid channel');
const gh=(...args)=>execFileSync('gh',args,{encoding:'utf8'});
const release=JSON.parse(gh('api',`repos/${repo}/releases/tags/channels`));
const index=JSON.parse(release.body||'{}');delete index[channel];writeFileSync('pause-body.json',JSON.stringify({body:JSON.stringify(index,null,2)}));
gh('api','--method','PATCH',`repos/${repo}/releases/${release.id}`,'--input','pause-body.json');
if(release.assets.some(a=>a.name===`${channel}.json`))gh('release','delete-asset','channels',`${channel}.json`,'--repo',repo,'--yes');
