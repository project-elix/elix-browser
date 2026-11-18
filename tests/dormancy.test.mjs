import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {describe,expect,it} from 'vitest';

const script=readFileSync(new URL('../src-tauri/src/browser/dormancy.js',import.meta.url),'utf8');

function fixture(){
 class Peer {signalingState='stable'}
 class Track {readyState='live';clone(){return new Track()}}
 const track=new Track();
 class Devices {getUserMedia(){return Promise.resolve({getTracks:()=>[track]})}}
 const media=[];
 const window={RTCPeerConnection:Peer,MediaDevices:Devices,MediaStreamTrack:Track};
 runInNewContext(script,{window,navigator:{},document:{querySelectorAll:()=>media}});
 return {window,media,track};
}
describe('smart tab dormancy',()=>{
 it('keeps a silent peer connection awake through a meeting',()=>{
  const {window}=fixture();
  expect(window.__elixFrameIdle()).toBe(true);
  const peer=new window.RTCPeerConnection();
  expect(window.__elixFrameIdle()).toBe(false);
  peer.signalingState='closed';
  expect(window.__elixFrameIdle()).toBe(true);
 });
 it('guards pending capture, live tracks and clones until all have ended',async()=>{
  const {window,track}=fixture();
  const capture=new window.MediaDevices().getUserMedia();
  expect(window.__elixFrameIdle()).toBe(false);
  await capture;
  expect(window.__elixFrameIdle()).toBe(false);
  const copy=track.clone();
  track.readyState='ended';
  expect(window.__elixFrameIdle()).toBe(false);
  copy.readyState='ended';
  expect(window.__elixFrameIdle()).toBe(true);
 });
 it('follows the actual media play state',()=>{
  const {window,media}=fixture();
  media.push({paused:false,ended:false});
  expect(window.__elixFrameIdle()).toBe(false);
  media[0].paused=true;
  expect(window.__elixFrameIdle()).toBe(true);
 });
});
