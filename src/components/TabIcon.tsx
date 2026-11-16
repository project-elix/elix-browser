import {useState} from 'react';
import {Globe,Home,Pause,RotateCw} from 'lucide-react';
import type {Tab} from '../types';
export default function TabIcon({tab}:{tab:Tab}){
 const[failed,setFailed]=useState<string|null>(null);
 if(tab.loading)return <RotateCw size={14} className="spin"/>;
 if(tab.suspended)return <Pause size={14}/>;
 if(tab.url.startsWith('elix:'))return <Home size={14}/>;
 const icon=tab.favicon;
 return icon&&icon!==failed&&/^data:image\/(png|jpeg|gif|webp|x-icon|vnd\.microsoft\.icon);base64,/i.test(icon)?<img className="tab-favicon" draggable={false} src={icon} alt="" onError={()=>setFailed(icon)}/>:<Globe size={14}/>;
}
