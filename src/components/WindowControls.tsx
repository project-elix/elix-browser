import {useEffect,useState} from 'react';
import {listen} from '@tauri-apps/api/event';
import {Minus,Square,Copy,Maximize,Minimize,X} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
type WindowState={maximized:boolean;fullscreen:boolean};
export default function WindowControls({onError}:{onError:(e:unknown)=>void}){
 const[state,setState]=useState<WindowState>({maximized:false,fullscreen:false});
 useEffect(()=>{if(!desktop)return;let disposed=false;const off=listen<WindowState>('window-state',e=>{if(!disposed)setState(e.payload)});void rpc<WindowState>('window',{operation:'state'}).then(s=>{if(!disposed)setState(s)}).catch(onError);return()=>{disposed=true;void off.then(fn=>fn())}},[onError]);
 const act=(operation:string)=>void rpc<WindowState>('window',{operation}).then(setState).catch(onError);
 return <div className="window-controls" aria-label="Vinduskontroller"><button title="Fullskjerm · F11" aria-label={state.fullscreen?'Avslutt fullskjerm':'Fullskjerm'} disabled={!desktop} onClick={()=>act('toggle_fullscreen')}>{state.fullscreen?<Minimize size={14}/>:<Maximize size={14}/>}</button><button title="Minimer" aria-label="Minimer vindu" disabled={!desktop} onClick={()=>act('minimize')}><Minus size={16}/></button><button title={state.maximized?'Gjenopprett vindu':'Maksimer vindu'} aria-label={state.maximized?'Gjenopprett vindu':'Maksimer vindu'} disabled={!desktop} onClick={()=>act('toggle_maximize')}>{state.maximized?<Copy size={13}/>:<Square size={13}/>}</button><button className="window-close" title="Lukk ELIX" aria-label="Lukk ELIX" disabled={!desktop} onClick={()=>act('close')}><X size={17}/></button></div>
}
