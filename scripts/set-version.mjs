import{readFileSync,writeFileSync}from'node:fs';import{validateVersion}from'./release-lib.mjs';
const version=process.argv[2],channel=process.argv[3]??'stable';validateVersion(version,channel);
for(const file of ['package.json','src-tauri/tauri.conf.json']){const v=JSON.parse(readFileSync(file));v.version=version;writeFileSync(file,JSON.stringify(v,null,2)+'\n');}
const file='src-tauri/Cargo.toml';writeFileSync(file,readFileSync(file,'utf8').replace(/^version = "[^"]+"/m,`version = "${version}"`));console.log('Version updated. Run npm install --package-lock-only and cargo update --workspace; commit both lockfiles.');
