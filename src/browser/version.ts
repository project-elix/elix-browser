export function releaseLabel(version:string){
 const alpha=/^\d+\.\d+\.\d+-dev\.(\d+)$/.exec(version);
 const beta=/^\d+\.\d+\.\d+-beta\.(\d+)$/.exec(version);
 return alpha?`Alpha ${alpha[1]} · ${version}`:beta?`Beta ${beta[1]} · ${version}`:version;
}
