// Only a bit is retained in each document. Field names, types and values never
// leave the page. A new document starts clean, including after native form submit.
(() => {
  let edited = false;
  const mark = event => { if (event.isTrusted) edited = true; };
  for (const name of ['beforeinput', 'input', 'change']) {
    window.addEventListener(name, mark, true);
  }
  Object.defineProperty(window, '__elixHasFieldEdits', {value: () => edited});
})();
