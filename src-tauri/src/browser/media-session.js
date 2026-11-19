// Media Session has no public getter for action handlers. Keep the two playlist
// callbacks as the page registers them, leaving all native media keys intact.
(() => {
  const session = navigator.mediaSession;
  if (!session || window.__elixPlaylist) return;
  const prototype = Object.getPrototypeOf(session);
  const descriptor = Object.getOwnPropertyDescriptor(prototype, 'setActionHandler');
  if (!descriptor?.value || !descriptor.configurable) return;
  const actions = new Map();
  Object.defineProperty(prototype, 'setActionHandler', {
    ...descriptor,
    value: function setActionHandler(action, handler) {
      const result = Reflect.apply(descriptor.value, this, arguments);
      if (this === session && (action === 'previoustrack' || action === 'nexttrack')) {
        if (typeof handler === 'function') actions.set(action, handler);
        else actions.delete(action);
      }
      return result;
    }
  });
  Object.defineProperty(window, '__elixPlaylist', {
    configurable: true,
    value: Object.freeze({
      has: action => actions.has(action),
      run(action) {
        const handler = actions.get(action);
        if (handler) handler({action});
      }
    })
  });
})();
