/* =====================================================================
   SKIRMISH II ADS — the whole ad system lives in this one file.
   ---------------------------------------------------------------------
   How it works
   - At most TWO ad slots per page (see ADS.md for the friendly
     rules). The landing page has #ad-slot-leaderboard (under the
     hero) and #ad-slot-play (beside the game showcase); the play
     page has two WAITING-view slots — #ad-slot-lobby (lobby
     sidebar) and #ad-slot-loading (loading screen) — which are
     never on screen at the same time and are both gone when a
     match starts. Slots never cover the canvas; there are no
     interstitials or pop-ups, ever.
   - The two waiting slots ROTATE while the player waits (M10):
     a fresh creative every ROTATE_MS (60 s), pausing while the
     tab is hidden and stopping the moment a match starts (the
     slots leave the screen — rotation only ever touches visible
     slots). In the house phase the rotation alternates the
     designed Skirmish II promo with the friendly house
     placeholder. Real-ad rotation is separately gated by
     CONFIG.rotateRealAds — read the policy note in ADS.md
     before ever turning it on.
   - While CONFIG.enabled is false — or the AdSense ids below are still
     placeholders — each slot renders a friendly HOUSE placeholder
     instead of a real ad (the lobby slot gets a designed promo for
     the game itself, so its size and placement read realistically).
   - To go live with AdSense: fill in clientId + the slot ids,
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
      "ad-slot-play": "0000000000",        // 300x250, beside the game
      "ad-slot-lobby": "0000000000",       // 300x250, /play lobby sidebar
      "ad-slot-loading": "0000000000"      // 300x250, /play loading screen
    },

    /* Real-ad rotation gate (M10). House placeholders rotate
       freely on the waiting slots; rotating a REAL AdSense unit
       is an auto-refresh and has policy constraints (minimum
       refresh interval, visibility rules) that change over time.
       Leave this false until the integrator has confirmed the
       CURRENT policy — see ADS.md, "Rotation". */
    rotateRealAds: false
  };

  /* The waiting-slot rotation interval (M10): one minute. Waits
     in the lobby/loading views run 5–15 minutes in the reference
     game; a fresh creative every 60 s keeps the wait alive
     without churning. It is also at the conservative end of
     historical auto-refresh minimums — still confirm the current
     AdSense policy before enabling real rotation (above). */
  var ROTATE_MS = 60000;

  /* Only the waiting views rotate: the slots a player stares at
     while a match is found and loaded. Landing slots are static. */
  var ROTATING_SLOTS = ["ad-slot-lobby", "ad-slot-loading"];

  var GITHUB_URL = "https://github.com/claudioolmedo/skirmish";

  /* The emblem sits next to this script (art/ in the site root) on
     every page that loads it — resolve it from the script's own URL
     so the path is right from / and from /play alike. */
  var EMBLEM_URL = "art/emblem.webp";
  try {
    var thisScript = document.currentScript;
    if (thisScript && thisScript.src) {
      EMBLEM_URL = thisScript.src.replace(/[^/]*$/, "") + "art/emblem.webp";
    }
  } catch (e) { /* keep the relative fallback */ }

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

  /* ----- lobby placeholder: a designed promo for the game itself --
     The lobby sidebar slot (M9) carries this simulated house ad until
     real units activate: emblem, headline and a button at the full
     300x250, so the placement reads exactly like a filled slot. It
     promotes Skirmish II itself — never a fake third-party sponsor. */

  function renderPromoPlaceholder(el) {
    el.classList.add("is-house");
    el.innerHTML =
      '<div class="promo-ad">' +
        '<img class="promo-emblem" src="' + EMBLEM_URL + '" alt="" width="52" height="52">' +
        '<p class="promo-title">SKIRMISH II</p>' +
        '<p class="promo-tag">The chess of the 21st century.</p>' +
        '<p class="promo-text">An open real-time strategy game inspired by the ' +
        'classics. Send a link, play a friend — right in the browser.</p>' +
        '<p class="promo-cta"><a href="' + GITHUB_URL + '">' +
        'Follow the build &rarr;</a></p>' +
        '<p class="promo-note">House ad — the game itself, for now</p>' +
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

  function isRotating(slotElementId) {
    return ROTATING_SLOTS.indexOf(slotElementId) !== -1;
  }

  function isWaitingSlot(slotElementId) {
    return slotElementId === "ad-slot-lobby" || slotElementId === "ad-slot-loading";
  }

  function initSlots() {
    var live = liveMode();
    if (live) loadAdsenseScript();
    Object.keys(CONFIG.slots).forEach(function (slotElementId) {
      var el = document.getElementById(slotElementId);
      if (!el) return;
      var slotId = CONFIG.slots[slotElementId];
      if (live && !isPlaceholder(slotId)) {
        renderRealAd(el, slotId);
      } else if (isWaitingSlot(slotElementId)) {
        // Both waiting slots open on the designed promo; the
        // rotation (below) alternates from there.
        el.setAttribute("data-creative", "promo");
        renderPromoPlaceholder(el);
      } else {
        renderHousePlaceholder(el);
      }
    });
    startRotation();
  }

  /* ---------- rotation during waits (M10) ----------

     One timer for the page. Each tick, every rotating slot that
     is actually ON SCREEN swaps its creative: house phase
     alternates the designed promo <-> the friendly house
     placeholder; live mode re-renders the real unit (only when
     CONFIG.rotateRealAds has been deliberately enabled after
     the policy check). The tick is a no-op while the tab is
     hidden, and a slot inside a hidden overlay (display:none —
     e.g. the lobby once a match starts) is skipped: rotation
     stops entirely the moment play begins, as it must. */

  function slotVisible(el) {
    // An element in a display:none subtree has no layout box.
    return el.offsetParent !== null || el.offsetWidth > 0;
  }

  function rotateSlot(el, slotElementId) {
    var live = liveMode();
    var slotId = CONFIG.slots[slotElementId];
    if (live && !isPlaceholder(slotId)) {
      if (CONFIG.rotateRealAds) renderRealAd(el, slotId);
      return;
    }
    var current = el.getAttribute("data-creative");
    if (current === "promo") {
      el.setAttribute("data-creative", "house");
      renderHousePlaceholder(el);
    } else {
      el.setAttribute("data-creative", "promo");
      renderPromoPlaceholder(el);
    }
  }

  function rotateTick() {
    if (typeof document !== "undefined" && document.hidden) return;
    ROTATING_SLOTS.forEach(function (slotElementId) {
      var el = document.getElementById(slotElementId);
      if (!el || !slotVisible(el)) return;
      rotateSlot(el, slotElementId);
    });
  }

  var rotationTimer = null;
  function startRotation() {
    if (rotationTimer !== null) return;
    rotationTimer = setInterval(rotateTick, ROTATE_MS);
  }

  /* Verification hook: lets the test suite (and a human in the
     console) drive one rotation tick and read the interval,
     without waiting a minute or mocking timers. */
  window.SkirmishAds = {
    ROTATE_MS: ROTATE_MS,
    rotateNow: rotateTick,
    creativeOf: function (slotElementId) {
      var el = document.getElementById(slotElementId);
      return el ? el.getAttribute("data-creative") : null;
    }
  };

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
