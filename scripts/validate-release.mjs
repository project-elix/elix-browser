import{readFileSync}from'node:fs';import{validateVersion}from'./release-lib.mjs';
const pkg=JSON.parse(readFileSync('package.json'));const config=JSON.parse(readFileSync('src-tauri/tauri.conf.json'));const cargo=readFileSync('src-tauri/Cargo.toml','utf8').match(/^version = "([^"]+)"/m)?.[1];
if(pkg.version!==config.version||pkg.version!==cargo)throw new Error('Version mismatch between package.json, Cargo.toml, tauri.conf.json');
if(process.env.RELEASE_VERSION){if(process.env.RELEASE_VERSION!==pkg.version)throw new Error('Requested version does not match committed source');validateVersion(pkg.version,process.env.RELEASE_CHANNEL);}
if(process.env.ELIX_ACCOUNTS_URL){
 const u=new URL(process.env.ELIX_ACCOUNTS_URL);
 if(u.protocol!=='https:'||u.username||u.password||u.pathname!=='/'||u.search||u.hash)throw new Error('ELIX_ACCOUNTS_URL must be an HTTPS origin without credentials, path or query');
}
if(process.env.REQUIRE_RELEASE_SECRETS==='true'){
 for(const key of ['TAURI_SIGNING_PRIVATE_KEY','ELIX_UPDATE_PUBLIC_KEY','ELIX_UPDATE_BASE'])if(!process.env[key])throw new Error(`Missing ${key}`);
 const u=new URL(process.env.ELIX_UPDATE_BASE);if(u.protocol!=='https:')throw new Error('HTTPS updater required');
 if(!process.env.ELIX_UPDATE_PUBLIC_KEY.trim())throw new Error('Missing public key');
}
console.log(`Validated ELIX Browser ${pkg.version}`);
