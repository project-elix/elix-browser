// The official channel Release body mirrors the published manifests. GitHub API supports browser CORS.
const repo='project-elix/elix-browser';
const status=document.getElementById('release-status');
try {
 const response=await fetch(`https://api.github.com/repos/${repo}/releases/tags/channels`,{headers:{Accept:'application/vnd.github+json'},cache:'no-store'});
 if(!response.ok)throw new Error('No public channel');const release=await response.json();
 const manifest=JSON.parse(release.body||'{}').stable;if(!manifest)throw new Error('Stable channel paused or not published');
 const platform=manifest.platforms?.['windows-x86_64'];const url=new URL(platform?.url);
 if(!/^\d+\.\d+\.\d+$/.test(manifest.version)||!platform.signature||url.protocol!=='https:'||url.hostname!=='github.com'||!url.pathname.startsWith(`/${repo}/releases/download/v${manifest.version}/`)||!url.pathname.endsWith('.exe'))throw new Error('Unverified release metadata');
 document.getElementById('release-pill').textContent='STABLE';document.getElementById('release-version').textContent=`ELIX Browser ${manifest.version}`;document.getElementById('release-date').textContent=`Utgitt ${new Date(manifest.pub_date).toLocaleDateString('nb-NO')}`;
 const button=document.getElementById('download-button');button.href=url.href;button.classList.remove('disabled');button.removeAttribute('aria-disabled');
 document.getElementById('release-notes').textContent=manifest.notes||'Ingen endringslogg oppgitt.';status.textContent='Offisiell Windows x64-installasjonsfil. Oppdateringer verifiseres med digital signatur i ELIX.';
} catch {status.textContent='Ingen bekreftet stabil nedlasting er tilgjengelig akkurat nå. Se GitHub for prosjektstatus.';}
