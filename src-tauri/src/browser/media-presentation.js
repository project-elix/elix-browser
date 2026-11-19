// Executed only after the shell requests a popout. This never receives native IPC,
// URLs or credentials. The original media element, source and listeners stay put.
(function present(preferred) {
  const key = '__elixMediaPresentation';
  const previous = window[key];
  const roots = [], seen = new Set(), candidates = [];
  const scan = root => {
    if (!root || seen.has(root) || seen.size > 100) return;
    seen.add(root); roots.push(root);
    for (const element of root.querySelectorAll('*')) {
      if (element.localName === 'video' || element.localName === 'audio') {
        const rect = element.getBoundingClientRect();
        // Prefer the playing element, then a loaded/visible player. Do not let
        // a tiny paused advertisement beat the site's actual playing audio.
        const loaded = !!(element.currentSrc || element.srcObject || element.readyState);
        if (loaded) candidates.push({element, score: (!element.paused && !element.ended ? 1e12 : 0)
          + (element.readyState > 1 ? 1e9 : 0) + Math.min(rect.width * rect.height, 1e8)});
      }
      if (element.shadowRoot) scan(element.shadowRoot);
      if (element.localName === 'iframe') {
        try { scan(element.contentDocument); } catch (_) { /* cross-origin stays isolated */ }
      }
    }
  };
  scan(document);
  candidates.sort((a, b) => b.score - a.score);
  const media = candidates.find(item => item.element === preferred)?.element || candidates[0]?.element;
  if (!media) return {ok: false, reason: 'no-media'};
  if (previous && typeof previous.restore === 'function') previous.restore();

  const changes = new Map(), emptyStyles = new Set(), styles = [], surfaces = [], scroll = [];
  let timer = 0, stopped = false, frameExited = false;
  const frameExits = [];
  const followPlaying = event => {
    const target = event.target;
    if (stopped || frameExited || target === media || !['audio', 'video'].includes(target.localName)) return;
    if (media.ended || media.paused || !media.isConnected) present(target);
  };
  const declarations = element => new Map(Array.from(element.style, name => [name, {
    value: element.style.getPropertyValue(name), priority: element.style.getPropertyPriority(name)
  }]));
  const remember = (element, property, value) => {
    let properties = changes.get(element);
    if (!properties) {
      properties = new Map(); changes.set(element, properties);
      if (!element.hasAttribute('style')) emptyStyles.add(element);
    }
    // Snapshot changed longhands, not a raw shorthand or its input spelling.
    // CSSOM normalizes colors/lengths and shorthands can overwrite longhands.
    const before = declarations(element);
    element.style.setProperty(property, value, 'important');
    const after = declarations(element);
    for (const name of new Set([...before.keys(), ...after.keys()])) {
      const old = before.get(name) || {value: '', priority: ''};
      const next = after.get(name) || {value: '', priority: ''};
      if (old.value === next.value && old.priority === next.priority) continue;
      if (!properties.has(name)) properties.set(name, {...old});
      Object.assign(properties.get(name), {applied: next.value, appliedPriority: next.priority});
    }
  };
  const style = (element, properties) => {
    for (const [property, value] of Object.entries(properties)) remember(element, property, value);
  };
  const controls = media.getAttribute('controls');
  const restore = () => {
    if (stopped) return true;
    stopped = true; clearInterval(timer);
    for (const root of roots) root.removeEventListener('playing', followPlaying, true);
    for (const [win, handler] of frameExits) { try { win.removeEventListener('pagehide', handler); } catch (_) {} }
    for (const element of styles) element.remove();
    for (const element of surfaces) element.remove();
    for (const [element, properties] of changes) {
      for (const [property, original] of properties) {
        // Preserve unrelated site mutations made while playback continued.
        if (element.style.getPropertyValue(property) !== original.applied
          || element.style.getPropertyPriority(property) !== original.appliedPriority) continue;
        if (original.value) element.style.setProperty(property, original.value, original.priority);
        else element.style.removeProperty(property);
      }
      if (emptyStyles.has(element) && element.style.length === 0) element.removeAttribute('style');
    }
    if (controls === null) media.removeAttribute('controls'); else media.setAttribute('controls', controls);
    for (const [win, x, y] of scroll) { try { win.scrollTo(x, y); } catch (_) {} }
    if (window[key]?.restore === restore) delete window[key];
    return true;
  };
  window[key] = {restore, kind: media.localName};
  try {
    const documents = new Set(), path = [];
    let element = media;
    while (element) {
      path.push(element);
      const doc = element.ownerDocument;
      documents.add(doc);
      // A slotted player's clipping and positioning come from the shadow
      // wrapper around the slot, even though it lives in the host's light DOM.
      if (element.assignedSlot) element = element.assignedSlot;
      else if (element.parentElement) element = element.parentElement;
      else {
        const root = element.getRootNode();
        element = root.host || (doc.defaultView && doc.defaultView.frameElement);
      }
    }
    // Hide page chrome in every accessible tree, including shadow DOM. The
    // selected element stays in its original DOM position with the same stream.
    for (const root of roots) {
      const sheet = root.ownerDocument?.createElement('style') || root.createElement('style');
      sheet.textContent = '* { visibility: hidden !important; } *::before, *::after { visibility: hidden !important; }';
      (root.head || root).appendChild(sheet); styles.push(sheet);
    }
    for (const doc of documents) {
      const win = doc.defaultView;
      scroll.push([win, win.scrollX, win.scrollY]);
      const surface = doc.createElement('div');
      surface.setAttribute('data-elix-media-surface', '');
      surface.setAttribute('aria-hidden', 'true');
      surface.style.cssText = 'position:fixed!important;inset:0!important;width:100vw!important;height:100vh!important;background:#080b12!important;visibility:visible!important;z-index:2147483646!important;pointer-events:none!important;display:flex!important;align-items:center!important;justify-content:center!important;color:#eaf3ff!important;font:600 18px system-ui!important;text-align:center!important;padding:24px!important;box-sizing:border-box!important;';

      (doc.body || doc.documentElement).appendChild(surface); surfaces.push(surface);
    }
    // Sites own their playlist. Forward only advertised Media Session actions,
    // including handlers on the embedding document for same-origin players.
    const mediaDoc = media.ownerDocument;
    const trackBar = mediaDoc.createElement('div');
    trackBar.setAttribute('data-elix-track-controls', '');
    trackBar.style.cssText = 'all:initial!important;position:fixed!important;right:12px!important;top:12px!important;z-index:2147483647!important;visibility:visible!important;display:block!important;';
    const shadow = trackBar.attachShadow({mode: 'open'});
    const trackStyle = mediaDoc.createElement('style');
    trackStyle.textContent = 'button{color:white;background:#172231eb;border:1px solid #587084;border-radius:6px;padding:6px 9px;margin-left:4px;cursor:pointer;font:600 13px system-ui}button:focus-visible{outline:2px solid white}button[hidden]{display:none}';
    shadow.append(trackStyle);
    const findPlaylist = action => {
      for (const doc of documents) {
        const playlist = doc.defaultView?.__elixPlaylist;
        if (playlist?.has(action)) return playlist;
      }
    };
    const buttons = new Map();
    for (const [action, label] of [['previoustrack', 'Forrige spor'], ['nexttrack', 'Neste spor']]) {
      const button = mediaDoc.createElement('button');
      button.type = 'button';
      button.textContent = label;
      button.hidden = true;
      button.addEventListener('click', () => findPlaylist(action)?.run(action));
      shadow.append(button);
      buttons.set(action, button);
    }
    // The player and buttons share a stacking context even inside a transformed
    // ancestor. The controls follow the media in painting order.
    (media.assignedSlot?.parentNode || media.parentNode).appendChild(trackBar);
    styles.push(trackBar);
    for (const root of roots) root.addEventListener('playing', followPlaying, true);
    const neutral = {
      transform: 'none', translate: 'none', rotate: 'none', scale: 'none', filter: 'none',
      'backdrop-filter': 'none', perspective: 'none', contain: 'none', 'content-visibility': 'visible',
      'will-change': 'auto', 'clip-path': 'none', clip: 'auto', overflow: 'visible', opacity: '1', display: 'block', position: 'static',
      isolation: 'auto', 'z-index': 'auto', 'mask-image': 'none', transition: 'none', animation: 'none'
    };
    const fill = {
      position: 'fixed', inset: '0px', width: '100vw', height: '100vh',
      'min-width': '0px', 'min-height': '0px', 'max-width': 'none', 'max-height': 'none',
      margin: '0px', padding: '0px', border: '0px', 'border-radius': '0px',
      'box-sizing': 'border-box', display: 'block', visibility: 'visible', 'z-index': '2147483647',
      'object-fit': 'contain', background: '#080b12', 'pointer-events': 'auto'
    };
    const markUnavailable = () => {
      for (const button of buttons.values()) button.hidden = true;
      for (const ancestor of path) if (ancestor.localName === 'iframe') style(ancestor, {visibility: 'hidden'});
      for (const surface of surfaces) surface.textContent = 'Spilleren ble endret. Gå tilbake til fanen og åpne minispilleren igjen.';
    };
    for (const doc of documents) {
      if (doc === document) continue;
      const onFrameExit = () => { frameExited = true; markUnavailable(); };
      doc.defaultView.addEventListener('pagehide', onFrameExit);
      frameExits.push([doc.defaultView, onFrameExit]);
    }
    const enforce = () => {
      if (stopped) return;
      for (const ancestor of path) {
        if (!ancestor.isConnected) continue;
        style(ancestor, neutral);
        if (ancestor === ancestor.ownerDocument.documentElement || ancestor === ancestor.ownerDocument.body)
          style(ancestor, {margin: '0px', padding: '0px', overflow: 'hidden', background: '#080b12'});
        if (ancestor.localName === 'iframe') style(ancestor, fill);
      }
      let currentDocument = false;
      try { currentDocument = media.ownerDocument.defaultView.document === media.ownerDocument; } catch (_) {}
      if (frameExited || !media.isConnected || !currentDocument) {
        // Independent frame navigation/replacement cannot expose a whole site.
        // A playlist can replace its media element between songs. Adopt the
        // replacement without starting, pausing or copying either stream.
        if (!frameExited && currentDocument && !media.isConnected && roots.some(root => root.querySelector('audio,video')) && present().ok) return;
        markUnavailable();
        return;
      }
      for (const [action, button] of buttons) button.hidden = !findPlaylist(action);
      if (media.localName === 'audio') {
        const metadata = mediaDoc.defaultView.navigator.mediaSession?.metadata;
        const title = `${metadata?.title || mediaDoc.title || 'Musikk'}${metadata?.artist ? ` · ${metadata.artist}` : ''}`;
        const surface = surfaces.find(item => item.ownerDocument === mediaDoc);
        if (surface.textContent !== title) surface.textContent = title;
      }
      style(media, fill);
      if (media.localName === 'audio') style(media, {inset: 'auto 24px 24px', width: 'calc(100vw - 48px)', height: '54px', background: 'transparent'});
      if (!media.hasAttribute('controls')) media.setAttribute('controls', '');
    };
    enforce();
    timer = window.setInterval(enforce, 500);
    return {ok: true, kind: media.localName};
  } catch (_) {
    restore();
    return {ok: false, reason: 'presentation-failed'};
  }
})()
