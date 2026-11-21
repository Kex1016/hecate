// haiiro: cut Zen's own scripted motion. Goes into the autoconfig
// (wrapFirefox extraPrefs); web pages keep their real prefers-reduced-motion.
Cc["@mozilla.org/observer-service;1"]
  .getService(Ci.nsIObserverService)
  .addObserver(win => {
    win.gReduceMotionOverride = true;
    const ui = win.gZenUIManager;
    if (ui?.motion) {
      ui.motion.MotionGlobalConfig.skipAnimations = true;
    }
    // Glance and other springs ignore gReduceMotion; finish them at once.
    if (ui?.elementAnimate) {
      const animate = ui.elementAnimate;
      ui.elementAnimate = function (element, keyframes, timing, ...rest) {
        const cut = typeof timing === "object" ? { ...timing, duration: 0, delay: 0 } : 0;
        return animate.call(this, element, keyframes, cut, ...rest);
      };
    }
    if (win.gZenFolders) {
      win.gZenFolders._dontAnimateFolder = true;
    }
  }, "browser-delayed-startup-finished");
