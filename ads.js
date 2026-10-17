/* =====================================================================
   SKIRMISH II ADS — the whole ad system lives in this one file.
   ---------------------------------------------------------------------
   How it works
   - The page has exactly TWO ad slots (see ADS.md for the friendly
     rules): #ad-slot-leaderboard (under the hero) and #ad-slot-play
     (beside the game). Slots never cover the canvas; there are no
     interstitials or pop-ups, ever.
   - While CONFIG.enabled is false — or the AdSense ids below are still
     placeholders — each slot renders a friendly HOUSE placeholder
     instead of a real ad.
   - To go live with AdSense: fill in clientId + the two slot ids,
     set enabled to true, commit. Nothing else changes. Full steps in
     ADS.md.
   ===================================================================== */
(function () {
  "use strict";

  var CONFIG = {
    /* Master switch — keep false until AdSense is approved. */
    enabled: false,

    provider: "adsense",

    /* AdSense publisher id, e.g. "ca-pub-1234567890123456". */
    clientId: "ca-pub-XXXXXXXXXXXXXXXX",

    /* Ad unit ids created in the AdSense dashboard, one per slot. */
    slots: {
      "ad-slot-leaderboard": "0000000000", // 728x90, collapses to 320x50
      "ad-slot-play": "0000000000"         // 300x250, beside the game
    }
  };

  var GITHUB_URL = "https://github.com/claudioolmedo/skirmish";

  function isPlaceholder(value) {
    return !value || /X{4,}/.test(value) || /^0+$/.test(value);
  }

  function liveMode() {
    return CONFIG.enabled &&
      CONFIG.provider === "adsense" &&
      !isPlaceholder(CONFIG.clientId);
  }

  /* ---------- house placeholder (what visitors see today) ---------- */

  function renderHousePlaceholder(el) {
    el.classList.add("is-house");
    el.innerHTML =
      '<div class="house-ad">' +
        '<p class="house-title">Your friendly ad will live here soon</p>' +
        '<p class="house-text">For now — thanks for playing! Skirmish II is ' +
        'open, and small ads like this one will help pay for the servers ' +
        'and keep development going.</p>' +
        '<p class="house-link"><a href="' + GITHUB_URL + '">' +
        'Follow the build on GitHub &rarr;</a></p>' +
      '</div>';
  }

  /* ---------- real AdSense unit ---------- */

  var adsenseScriptLoaded = false;
  function loadAdsenseScript() {
    if (adsenseScriptLoaded) return;
    adsenseScriptLoaded = true;
    var s = document.createElement("script");
    s.async = true;
    s.src = "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client=" +
      encodeURIComponent(CONFIG.clientId);
    s.crossOrigin = "anonymous";
    document.head.appendChild(s);
  }

  function renderRealAd(el, slotId) {
    el.innerHTML = "";
    var ins = document.createElement("ins");
    ins.className = "adsbygoogle";
    ins.style.display = "block";
    ins.style.width = "100%";
    ins.style.height = "100%";
    ins.setAttribute("data-ad-client", CONFIG.clientId);
    ins.setAttribute("data-ad-slot", slotId);
    el.appendChild(ins);
    (window.adsbygoogle = window.adsbygoogle || []).push({});
  }

  /* ---------- wire up the slots ---------- */

  function initSlots() {
    var live = liveMode();
    if (live) loadAdsenseScript();
    Object.keys(CONFIG.slots).forEach(function (slotElementId) {
      var el = document.getElementById(slotElementId);
      if (!el) return;
      var slotId = CONFIG.slots[slotElementId];
      if (live && !isPlaceholder(slotId)) {
        renderRealAd(el, slotId);
      } else {
        renderHousePlaceholder(el);
      }
    });
  }

  /* ---------- friendly transparency notice (dismissible) ---------- */

  var NOTICE_KEY = "skirmish.adNoticeDismissed";

  function initNotice() {
    var notice = document.getElementById("ad-notice");
    if (!notice) return;
    try {
      if (window.localStorage.getItem(NOTICE_KEY) === "1") {
        notice.style.display = "none";
        return;
      }
    } catch (e) { /* storage unavailable — just keep the notice visible */ }
    var closeBtn = document.getElementById("ad-notice-close");
    if (closeBtn) {
      closeBtn.addEventListener("click", function () {
        notice.style.display = "none";
        try { window.localStorage.setItem(NOTICE_KEY, "1"); } catch (e) {}
      });
    }
  }

  function init() {
    initSlots();
    initNotice();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
