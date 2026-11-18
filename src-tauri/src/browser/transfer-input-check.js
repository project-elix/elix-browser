(() => {
  try {
    const inspect = frame => {
      if (typeof frame.__elixHasFieldEdits !== 'function') return 'unknown';
      if (frame.__elixHasFieldEdits()) return 'edited';
      for (let i = 0; i < frame.length; i++) {
        const state = inspect(frame.frames[i]);
        if (state !== 'clean') return state;
      }
      return 'clean';
    };
    return inspect(window);
  } catch (_) { return 'unknown'; }
})()
