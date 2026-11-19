"""Ordinary local web content used to observe the installed browser from the outside."""

from pathlib import Path
from urllib.parse import urlsplit


def pdf():
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        ("<< /Type /Pages /Count 24 /Kids [" + " ".join(f"{3 + i*2} 0 R" for i in range(24)) + "] >>").encode(),
    ]
    for index in range(24):
        color = "1 0 0" if index == 0 else "0 0 1" if index == 23 else "0 1 0"
        # A 36 pt yellow stripe lets the desktop screenshot prove a real zoom change.
        paint = f"{color} rg 0 0 600 900 re f\n1 1 0 rg 282 0 36 900 re f\n".encode()
        objects.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] /Resources << >> /Contents {4+index*2} 0 R >>".encode())
        objects.append(f"<< /Length {len(paint)} >>\nstream\n".encode() + paint + b"endstream")
    data = bytearray(b"%PDF-1.4\n")
    offsets = []
    for n, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{n} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(objects)+1}\n0000000000 65535 f \n".encode())
    for offset in offsets:
        data.extend(f"{offset:010d} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


MEDIA = """<!doctype html><meta charset="utf-8"><title>ELIX QA medieside</title>
<style>
body{margin:0;background:#20232c;color:white;font:20px system-ui}
header{padding:10px;background:#692337}
button{font:inherit;margin-right:12px}
section{transform:translate(7px,3px);overflow:hidden;width:65vw;height:55vh}
video{width:100%;height:100%}
</style>
<header id="chrome"><button id="start">Start testavspilling</button><button id="expand">Vis testvideo i fullskjerm</button>Testsidens meny</header>
<section><video id="video" playsinline controls></video></section>
<script>
const video=document.querySelector('video'), chromeBar=document.querySelector('header');
const token=crypto.randomUUID(), route=location.pathname;
let stream, ctx, meter, ticks=0, focusClicks=0;
const clip=new URL('/qa-tone.webm',location.href).href;
document.querySelector('#start').onclick=async()=>{
 if(route==='/qa-fullscreen'&&!stream){
   const canvas=document.createElement('canvas');canvas.width=320;canvas.height=180;
   const paint=canvas.getContext('2d');
   const draw=()=>{paint.fillStyle='#3078ae';paint.fillRect(0,0,320,180);paint.fillStyle='#fff';paint.fillRect((ticks++*4)%264,66,50,50)};
   draw();setInterval(draw,100);
   stream=canvas.captureStream(10);video.muted=true;video.srcObject=stream;
 }else if(route==='/qa-miniplayer'&&!ctx){
   // A regular decoded A/V resource produces real post-element PCM. Route
   // that output once through a unity-gain analyser to the normal destination.
   video.src=clip;video.loop=true;
   ctx=new AudioContext();meter=ctx.createAnalyser();meter.fftSize=4096;
   ctx.createMediaElementSource(video).connect(meter);meter.connect(ctx.destination);
 }
 if(ctx)await ctx.resume();
 await video.play();
};
// Ordinary, physically operated site shortcuts also work in the isolated
// view where the site's own toolbar is hidden. They change the media element.
video.addEventListener('contextmenu',event=>{
 if(route==='/qa-miniplayer'&&event.isTrusted){event.preventDefault();video.focus();focusClicks++}
});
document.addEventListener('keydown',event=>{
 if(route!=='/qa-miniplayer'||!event.isTrusted||event.repeat||event.ctrlKey||event.altKey||event.metaKey)return;
 const key=event.key.toLowerCase();
 if(key==='q'){event.preventDefault();event.stopPropagation();video.muted=!video.muted}
 if(key==='w'){event.preventDefault();event.stopPropagation();if(video.paused)video.play();else video.pause()}
},true);
function postElementPcm(){
 if(!meter)return null;
 const data=new Float32Array(meter.fftSize);meter.getFloatTimeDomainData(data);
 let energy=0,sine=0,cosine=0;
 for(let i=0;i<data.length;i++){
  const value=data[i],phase=2*Math.PI*440*i/ctx.sampleRate;
  energy+=value*value;sine+=value*Math.sin(phase);cosine+=value*Math.cos(phase);
 }
 return {rms:Math.sqrt(energy/data.length),toneFraction:energy?Math.min(1,2*(sine*sine+cosine*cosine)/(energy*data.length)):0,
   clock:ctx.currentTime,sampleRate:ctx.sampleRate,samples:data.length};
}
document.querySelector('#expand').onclick=()=>video.requestFullscreen();
const report=()=>{
 const r=video.getBoundingClientRect(), elements=[...document.querySelectorAll('audio,video')];
 const active=elements.filter(media=>!media.paused&&!media.ended&&!media.muted&&media.volume>0);
 const body={kind:'installed-media',route,token,playing:!video.paused,t:video.currentTime,
   focused:document.hasFocus(),focusClicks,frames:video.getVideoPlaybackQuality().totalVideoFrames,muted:video.muted,
   liveVideo:!!stream&&stream.getVideoTracks().filter(t=>t.enabled&&t.readyState==='live').length===1,
   fullscreen:document.fullscreenElement===video,siteChrome:getComputedStyle(chromeBar).visibility,
   sameStream:document.querySelector('#video')===video&&(route==='/qa-fullscreen'?!!stream&&stream===video.srcObject:video.currentSrc===clip),
   mediaCount:elements.length,audibleElements:active.length,pcm:postElementPcm(),
   liveAudio:!!meter&&ctx.state==='running',
   r:[r.x,r.y,r.width,r.height],viewport:[innerWidth,innerHeight],scale:devicePixelRatio};
 fetch('/event',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}).catch(()=>{});
};
setInterval(report,300);document.addEventListener('fullscreenchange',report);window.addEventListener('resize',report);report();
</script>"""


def response(path):
    path = urlsplit(path).path
    if path == "/qa-tone.webm":
        # 90 s of moving VP8 video and one mono 440 Hz Opus track, generated
        # from FFmpeg's testsrc2 (160x90, 8 fps) and sine (48 kHz) sources.
        return "video/webm", Path(__file__).with_name("qa-tone.webm").read_bytes()
    if path == "/qa-scroll.pdf":
        return "application/pdf", pdf()
    if path in ("/qa-fullscreen", "/qa-miniplayer"):
        return "text/html; charset=utf-8", MEDIA.encode()
    return None
