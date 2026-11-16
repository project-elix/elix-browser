export function releaseLabel(version:string){
 const alpha=/^\d+\.\d+\.\d+-dev\.(\d+)$/.exec(version);
 return alpha?`Alpha ${alpha[1]} · ${version}`:version;
}
