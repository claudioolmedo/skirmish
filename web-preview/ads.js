/* =====================================================================
   SKIRMISH II ADS — the whole ad system lives in this one file.
   ---------------------------------------------------------------------
   How it works
   - At most TWO ad slots per page (see ADS.md for the friendly
     rules). The landing page has #ad-slot-leaderboard (under the
     hero) and #ad-slot-play (beside the game showcase); the play
     page has one, #ad-slot-lobby, in the lobby sidebar — it lives
     only in the lobby/waiting view and is gone when a match starts.
     Slots never cover the canvas; there are no interstitials or
     pop-ups, ever.
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
      "ad-slot-lobby": "0000000000"        // 300x250, /play lobby sidebar
    }
  };

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

  function initSlots() {
    var live = liveMode();
    if (live) loadAdsenseScript();
    Object.keys(CONFIG.slots).forEach(function (slotElementId) {
      var el = document.getElementById(slotElementId);
      if (!el) return;
      var slotId = CONFIG.slots[slotElementId];
      if (live && !isPlaceholder(slotId)) {
        renderRealAd(el, slotId);
      } else if (slotElementId === "ad-slot-lobby") {
        renderPromoPlaceholder(el);
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
