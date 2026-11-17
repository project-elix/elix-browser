// Executed only after the shell requests a popout. This never receives native IPC,
// URLs or credentials. The original media element, source and listeners stay put.
(() => {
  const key = '__elixMediaPresentation';
  const previous = window[key];
  if (previous && typeof previous.restore === 'function') previous.restore();
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
  const media = candidates[0]?.element;
  if (!media) return {ok: false, reason: 'no-media'};

  const changes = new Map(), emptyStyles = new Set(), styles = [], surfaces = [], scroll = [];
  let timer = 0, stopped = false, unavailable = false;
  const frameExits = [];
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
      if (element.parentElement) element = element.parentElement;
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
      if (doc === media.ownerDocument && media.localName === 'audio') {
        const metadata = win.navigator.mediaSession?.metadata;
        surface.textContent = `${metadata?.title || doc.title || 'Musikk'}${metadata?.artist ? ` · ${metadata.artist}` : ''}`;
      }
      (doc.body || doc.documentElement).appendChild(surface); surfaces.push(surface);
    }
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
      unavailable = true;
      for (const ancestor of path) if (ancestor.localName === 'iframe') style(ancestor, {visibility: 'hidden'});
      for (const surface of surfaces) surface.textContent = 'Spilleren ble endret. Gå tilbake til fanen og åpne minispilleren igjen.';
    };
    for (const doc of documents) {
      if (doc === document) continue;
      doc.defaultView.addEventListener('pagehide', markUnavailable);
      frameExits.push([doc.defaultView, markUnavailable]);
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
      if (unavailable || !media.isConnected || !currentDocument) {
        // Independent frame navigation/replacement cannot expose a whole site.
        markUnavailable();
        return;
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
