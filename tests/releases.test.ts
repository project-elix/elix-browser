import{describe,it,expect}from'vitest';
// @ts-expect-error Plain JS release utility is exercised as used by CI.
import{validateVersion,makeManifest,isNewer,findDraft,verifiedReleaseSource,findVerifiedRelease,publishedInstallerUrl}from'../scripts/release-lib.mjs';
describe('release channel safety',()=>{it('rejects cross-channel versions and shell input',()=>{for(const [v,c]of[['0.1.0-beta.1','stable'],['0.1.0','beta'],['0.1.0-beta.1','developer'],['0.1.0;whoami','stable'],['01.1.0','stable']])expect(()=>validateVersion(v,c)).toThrow()});it('accepts only defined channel versions',()=>{expect(validateVersion('0.1.0','stable')).toBe('0.1.0');expect(validateVersion('0.2.0-beta.1','beta')).toBe('0.2.0-beta.1');expect(validateVersion('0.2.0-dev.1','developer')).toBe('0.2.0-dev.1')});it('never creates unsigned or HTTP manifests',()=>{expect(()=>makeManifest({version:'0.1.0',channel:'stable',url:'http://example.com/a.exe',signature:'x'})).toThrow();expect(()=>makeManifest({version:'0.1.0',channel:'stable',url:'https://example.com/a.exe',signature:''})).toThrow()});it('rejects a signature bound to another version',()=>{expect(()=>makeManifest({version:'0.1.1',channel:'stable',url:'https://example.com/a.exe',signature:btoa('trusted comment: version:0.1.0')})).toThrow()})});

it('requires monotonically increasing channel versions',()=>{expect(isNewer('0.1.1','0.1.0')).toBe(true);expect(isNewer('0.1.0','0.1.1')).toBe(false);expect(isNewer('0.2.0','0.2.0-beta.3')).toBe(true);expect(isNewer('0.2.0-beta.1','0.2.0')).toBe(false);});

const source='a'.repeat(40);
it('finds only the unpublished draft built from the verified source',()=>{
 const draft={tag_name:'v0.1.0-dev.3',draft:true,target_commitish:source};
 expect(findDraft([{tag_name:'older'},draft],draft.tag_name,source)).toBe(draft);
 for(const releases of [[],[draft,draft],[{...draft,draft:false}],[{...draft,target_commitish:'b'.repeat(40)}]])expect(()=>findDraft(releases,draft.tag_name,source)).toThrow();
});
it('refuses publication recovery without the original native, upgrade and signing gates',()=>{
 const run={id:123,path:'.github/workflows/release.yml',event:'workflow_dispatch',head_branch:'main',status:'completed',head_sha:source};
 const jobs=['native-verification / windows','upgrade-verification / upgrade','build'].map(name=>({name,run_id:123,conclusion:'success'}));
 expect(verifiedReleaseSource(run,jobs)).toBe(source);
 for(const field of ['path','event','head_branch','status','head_sha'])expect(()=>verifiedReleaseSource({...run,[field]:'invalid'},jobs)).toThrow();
 for(let i=0;i<jobs.length;i++){
  expect(()=>verifiedReleaseSource(run,jobs.filter((_,n)=>n!==i))).toThrow();
  expect(()=>verifiedReleaseSource(run,jobs.map((j,n)=>n===i?{...j,conclusion:'failure'}:j))).toThrow();
 }
 expect(()=>verifiedReleaseSource(run,jobs.map(j=>({...j,run_id:456})))).toThrow();
});

it('uses the final published URL and refuses temporary draft asset links',()=>{
 const repo='project-elix/elix-browser',version='0.1.0-dev.3';
 const url=`https://github.com/${repo}/releases/download/v${version}/ELIX.exe`;
 const release={draft:false,tag_name:`v${version}`,target_commitish:source,assets:[{id:7,browser_download_url:url}]};
 expect(publishedInstallerUrl(release,7,repo,version)).toBe(url);
 expect(findVerifiedRelease([release],release.tag_name,source,false)).toBe(release);
 expect(()=>findDraft([release],release.tag_name,source)).toThrow();
 for(const badUrl of [url.replace(`v${version}`,'untagged-temporary'),url.replace('https:','http:'),url.replace('github.com','evil.example')])expect(()=>publishedInstallerUrl({...release,assets:[{id:7,browser_download_url:badUrl}]},7,repo,version)).toThrow();
 expect(()=>publishedInstallerUrl({...release,draft:true},7,repo,version)).toThrow();
 expect(()=>publishedInstallerUrl(release,8,repo,version)).toThrow();
});
