"""Ordinary local web content used to observe the installed browser from the outside."""

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
let stream, ctx, ticks=0;
document.querySelector('#start').onclick=async()=>{
 if(!stream){
   const canvas=document.createElement('canvas');canvas.width=320;canvas.height=180;
   const paint=canvas.getContext('2d');
   const draw=()=>{paint.fillStyle='#3078ae';paint.fillRect(0,0,320,180);paint.fillStyle='#fff';paint.fillRect((ticks++*4)%264,66,50,50)};
   draw();setInterval(draw,100);
   stream=canvas.captureStream(10);
   if(route==='/qa-fullscreen'){
     video.muted=true;
   }else{
     // Exactly one output route. The oscillator feeds the video's original
     // MediaStream and has no separate connection to the speaker destination.
     ctx=new AudioContext();const tone=ctx.createOscillator(),gain=ctx.createGain(),dest=ctx.createMediaStreamDestination();
     tone.frequency.value=440;gain.gain.value=0.12;tone.connect(gain);gain.connect(dest);tone.start();
     for(const track of dest.stream.getAudioTracks())stream.addTrack(track);
   }
   video.srcObject=stream;
 }
 if(ctx)await ctx.resume();
 await video.play();
};
document.querySelector('#expand').onclick=()=>video.requestFullscreen();
const report=()=>{
 const r=video.getBoundingClientRect(), elements=[...document.querySelectorAll('audio,video')];
 const active=elements.filter(media=>!media.paused&&!media.ended&&!media.muted&&media.volume>0);
 const body={kind:'installed-media',route,token,playing:!video.paused,t:video.currentTime,
   frames:video.getVideoPlaybackQuality().totalVideoFrames,muted:video.muted,
   liveVideo:!!stream&&stream.getVideoTracks().filter(t=>t.enabled&&t.readyState==='live').length===1,
   fullscreen:document.fullscreenElement===video,siteChrome:getComputedStyle(chromeBar).visibility,
   sameStream:!!stream&&stream===video.srcObject,mediaCount:elements.length,audibleElements:active.length,
   liveAudio:!!stream&&stream.getAudioTracks().filter(t=>t.enabled&&t.readyState==='live').length===1,
   r:[r.x,r.y,r.width,r.height],viewport:[innerWidth,innerHeight],scale:devicePixelRatio};
 fetch('/event',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)}).catch(()=>{});
};
setInterval(report,300);document.addEventListener('fullscreenchange',report);window.addEventListener('resize',report);report();
</script>"""


def response(path):
    path = urlsplit(path).path
    if path == "/qa-scroll.pdf":
        return "application/pdf", pdf()
    if path in ("/qa-fullscreen", "/qa-miniplayer"):
        return "text/html; charset=utf-8", MEDIA.encode()
    return None
