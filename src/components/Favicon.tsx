import {useState} from 'react';
import {Globe} from 'lucide-react';
export default function Favicon({icon,size=16}:{icon?:string|null;size?:number}){
 const [failed,setFailed]=useState<string|null>(null);
 return icon&&icon!==failed&&icon.length<700000&&/^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(icon)?<img className="tab-favicon" width={size} height={size} src={icon} alt="" onError={()=>setFailed(icon)}/>:<Globe size={size}/>;
}
