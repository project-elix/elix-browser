import {useEffect,useRef,useState} from 'react';
import {listen} from '@tauri-apps/api/event';
import {PictureInPicture2,Eye,EyeOff,CornerUpLeft,Volume2,VolumeX} from 'lucide-react';
import {desktop,rpc} from '../browser/bridge';
import type {Tab} from '../types';
import './media.css';

interface MediaState {player:{id:string;hidden:boolean}|null;fullscreen:string|null}
const empty:MediaState={player:null,fullscreen:null};

export default function MediaControls({active,tabs,compact,onError}:{active?:Tab;tabs:Tab[];compact:boolean;onError:(error:unknown)=>void}){
 const [state,setState]=useState<MediaState>(empty),[busy,setBusy]=useState(false);
 const revision=useRef(0);
 useEffect(()=>{
  if(!desktop)return;
  let disposed=false,off:(()=>void)|undefined;
  void listen<MediaState>('media-changed',event=>{if(!disposed){revision.current++;setState(event.payload)}}).then(async unsubscribe=>{
   if(disposed){unsubscribe();return}off=unsubscribe;
   const before=revision.current,value=await rpc<MediaState>('media',{operation:'state'});
   if(!disposed&&revision.current===before)setState(value);
  }).catch(error=>{if(!disposed)onError(error)});
  return()=>{disposed=true;off?.()};
 },[onError]);
 const action=async(operation:string)=>{
  setBusy(true);
  try{await rpc<MediaState>('media',{operation,...(operation==='open'?{id:active?.id}:{})})}
  catch(error){onError(error)}finally{setBusy(false)}
 };
 const player=tabs.find(tab=>tab.id===state.player?.id);
 return <section className={`rail-media ${compact?'rail-media-compact':''}`} aria-label="Minispiller">
  <button className="rail-tool" title="Åpne video eller musikk i minispiller" aria-label="Åpne video eller musikk i minispiller" disabled={busy||!desktop||!active||active.url.startsWith('elix:')||active.suspended} onClick={()=>void action('open')}><PictureInPicture2 size={19}/><span>Minispiller</span></button>
  {!compact&&!state.player&&<p>Vis bare videoen eller musikken i et vindu som kan endres i størrelse og minimeres.</p>}
  {state.player&&<>
   {!compact&&<p className="media-title" title={player?.title}>{player?.private?'Privat minispiller':player?.title||'Minispiller'}<small>{state.player.hidden?'Skjult · avspillingen fortsetter':'Eget vindu · avspillingen fortsetter'}</small></p>}
   <button className="rail-tool" title={state.player.hidden?'Vis minispiller':'Skjul minispiller – behold lyden'} aria-label={state.player.hidden?'Vis minispiller':'Skjul minispiller – behold lyden'} disabled={busy} onClick={()=>void action(state.player!.hidden?'show':'hide')}>{state.player.hidden?<Eye size={18}/>:<EyeOff size={18}/>}<span>{state.player.hidden?'Vis spiller':'Skjul spiller'}</span></button>
   {player&&<button className="rail-tool" title={player.muted?'Slå på lyd i minispiller':'Demp minispiller'} aria-label={player.muted?'Slå på lyd i minispiller':'Demp minispiller'} onClick={()=>void rpc('mute_tab',{id:player.id,muted:!player.muted}).catch(onError)}>{player.muted?<VolumeX size={18}/>:<Volume2 size={18}/>}<span>{player.muted?'Slå på lyd':'Demp spiller'}</span></button>}
   <button className="rail-tool" title="Flytt minispiller tilbake til fanen" aria-label="Flytt minispiller tilbake til fanen" disabled={busy} onClick={()=>void action('return')}><CornerUpLeft size={18}/><span>Tilbake til fanen</span></button>
  </>}
 </section>;
}
