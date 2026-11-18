// Per-document activity that WebView2's native audio indicator cannot see,
// including a silent meeting, a pending capture request, or screen sharing.
// This script has no IPC, profile data or access across origins.
(() => {
  const tracks = new Set(), peers = new Set();
  let pending = 0;
  const live = (entries, active) => {
    let found = false;
    for (const ref of entries) {
      const item = ref.deref();
      if (item && active(item)) found = true;
      else entries.delete(ref);
    }
    return found;
  };
  const rememberStream = stream => {
    for (const track of stream.getTracks()) tracks.add(new WeakRef(track));
    return stream;
  };
  const wrap = (obj, key, handler) => {
    const descriptor = obj && Object.getOwnPropertyDescriptor(obj, key);
    if (typeof descriptor?.value === 'function') {
      Object.defineProperty(obj, key, {...descriptor, value:new Proxy(descriptor.value, handler)});
    }
  };
  try {
    for (const name of ['getUserMedia', 'getDisplayMedia']) {
      wrap(window.MediaDevices?.prototype, name, {
        apply(method, receiver, args) {
          const result = Reflect.apply(method, receiver, args);
          pending++;
          return result.then(rememberStream).finally(() => pending--);
        }
      });
    }
    wrap(window.MediaStreamTrack?.prototype, 'clone', {
      apply(method, receiver, args) {
        const copy = Reflect.apply(method, receiver, args);
        tracks.add(new WeakRef(copy));
        return copy;
      }
    });
    wrap(window.MediaStream?.prototype, 'clone', {
      apply: (method, receiver, args) => rememberStream(Reflect.apply(method, receiver, args))
    });
    for (const name of ['RTCPeerConnection', 'webkitRTCPeerConnection']) {
      wrap(window, name, {
        construct(constructor, args, newTarget) {
          const peer = Reflect.construct(constructor, args, newTarget);
          peers.add(new WeakRef(peer));
          return peer;
        }
      });
    }
    Object.defineProperty(window, '__elixFrameIdle', {
      value: () => pending === 0 &&
        !live(tracks, track => track.readyState === 'live') &&
        !live(peers, peer => peer.signalingState !== 'closed') &&
        navigator.mediaSession?.playbackState !== 'playing' &&
        !Array.from(document.querySelectorAll('audio,video')).some(media => !media.paused && !media.ended)
    });
  } catch (_) { /* Unknown activity keeps this document awake. */ }
})();
