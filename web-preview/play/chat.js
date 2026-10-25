// Skirmish II — direct player-to-player chat (2026-10-24,
// Claudio's request).
//
// A DE-style chat between the two players of a 1v1 match that
// leaves NOTHING on the network: message text and taunt codes
// travel only over a WebRTC RTCDataChannel — DTLS-encrypted,
// peer to peer. The match relay carries ONLY the WebRTC
// signaling handshake (the 'rtc' envelope in netplay.js: SDP
// descriptions + ICE candidates, the phone-book exchange any
// WebRTC call needs). Content is never sent over the relay
// socket, never written to storage, never put in a replay, and
// never counted by telemetry. If the data channel cannot be
// established, chat fails CLOSED — a plain notice, and no
// relay fallback, ever. The module is pure protocol/session
// logic with injected transport + RTC factory, like netplay.js,
// so the whole state machine is testable headlessly; app.js is
// the thin DOM integrator.
//
// Taunts (the addition Claudio asked for in the same breath):
// a bare number that matches TAUNTS below is not text — the
// SENDER's client plays its local sound at once and sends only
// the code; the receiver plays ITS local copy and both logs
// show the phrase. The sounds are ours (tools/sound-gen:
// original phrases in the project's voices, keys taunt.<code>
// in the generated bank). An unmapped number is ordinary text.

// The taunt map — DE's numbering semantics for the classics a
// DE player expects (public documentation), our own words and
// voices. Every code here has a generated sound (taunt.<code>)
// and a phrase; do not map a code we do not ship.
export const TAUNTS = {
  1: 'Yes.',
  2: 'No.',
  3: 'Food, please.',
  4: 'Wood, please.',
  5: 'Gold, please.',
  6: 'Stone, please.',
  7: 'Help! I am under attack!',
  11: 'Ha ha ha ha!',
  12: 'Send food, if you can spare it.',
  13: 'Send wood, if you can spare it.',
  14: 'Let the battle begin!',
  30: '*chanting*',
};
export const TAUNT_CODES = Object.keys(TAUNTS).map(Number);

export const CHAT_TEXT_MAX = 200; // chars, both directions
export const CHAT_SEND_MIN_MS = 400; // sender-side text pacing
export const TAUNT_COOLDOWN_MS = 2000; // per player, both sides
export const RTC_CONNECT_TIMEOUT_MS = 12000; // fail closed after this

// One public STUN server so two home routers can find each
// other (without it most NAT pairs never connect). A STUN
// lookup carries no chat content and no identity beyond the
// network address any WebRTC app exposes to its peer anyway;
// there is deliberately NO TURN server — a relayed media path
// is exactly what this design refuses.
export const RTC_CONFIG = {
  iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
};

const BARE_NUMBER = /^\d{1,3}$/;

/// Parse what a player typed (pure). Returns one of:
///   { kind: 'taunt', code }       — a mapped bare number
///   { kind: 'text', text }        — ordinary text (cleaned,
///                                   capped), incl. unmapped
///                                   bare numbers
///   null                          — nothing sendable
export function parseChatInput(raw) {
  if (typeof raw !== 'string') return null;
  const text = raw.replace(/[\u0000-\u001F\u007F]/g, '').trim().slice(0, CHAT_TEXT_MAX);
  if (!text) return null;
  if (BARE_NUMBER.test(text)) {
    const code = Number(text);
    if (Object.hasOwn(TAUNTS, code)) return { kind: 'taunt', code };
  }
  return { kind: 'text', text };
}

/// Validate + normalize a frame arriving over the data channel
/// (pure). The peer is trusted to be one of the two players,
/// never to be well-behaved: shape, caps and kinds are checked
/// exactly like the relay codec checks its envelopes.
export function decodeChatFrame(text) {
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    return null;
  }
  if (!msg || typeof msg !== 'object') return null;
  if (msg.t === 'text') {
    if (typeof msg.text !== 'string') return null;
    const clean = msg.text.replace(/[\u0000-\u001F\u007F]/g, '').trim().slice(0, CHAT_TEXT_MAX);
    return clean ? { kind: 'text', text: clean } : null;
  }
  if (msg.t === 'taunt') {
    if (!Number.isInteger(msg.code) || !Object.hasOwn(TAUNTS, msg.code)) return null;
    return { kind: 'taunt', code: msg.code };
  }
  return null;
}

/// Validate an inbound 'rtc' signaling envelope's payload for
/// the link (pure): the codec already bounded the shapes; this
/// checks the kind is one the link acts on.
export function isSignalMessage(msg) {
  return !!msg && msg.type === 'rtc'
    && (msg.kind === 'offer' || msg.kind === 'answer' || msg.kind === 'ice');
}

/**
 * One chat link to the match peer. Exactly one side offers —
 * the HOST — so two clients can never glare (both offering at
 * once); the guest answers. Signaling rides the injected
 * `sendSignal` (the room socket, via netplay's envelope);
 * content rides only the data channel.
 *
 * Events (onEvent): { type:'open' } the channel is live ·
 * { type:'unavailable' } connect timeout or channel/ICE
 * failure — chat is closed for this match, no fallback ·
 * { type:'closed' } the channel closed after being open.
 * Inbound content arrives via onMessage({kind:'text',text} |
 * {kind:'taunt',code}) — already validated.
 */
export function createChatLink({
  isHost,
  sendSignal,
  onMessage = () => {},
  onEvent = () => {},
  onLocalTaunt = () => {},
  rtcFactory = null,
  config = RTC_CONFIG,
  now = () => Date.now(),
  setTimeoutFn = (fn, ms) => setTimeout(fn, ms),
  clearTimeoutFn = (id) => clearTimeout(id),
  offerRetryMs = 2500,
  maxOfferRetries = 4,
}) {
  const factory = rtcFactory
    || (typeof globalThis !== 'undefined' ? globalThis.RTCPeerConnection : null);
  let pc = null;
  let channel = null;
  let state = 'idle'; // idle | connecting | open | unavailable | closed
  let connectTimer = null;
  let retryTimer = null;
  let answerSeen = false;
  let offerRetries = 0;
  let remoteSet = false;
  const pendingIce = [];
  let lastTextAt = -Infinity;
  let lastTauntAt = -Infinity; // own sends
  let lastPeerTauntAt = -Infinity; // inbound: spam lands as silence

  function failClosed() {
    if (state === 'open' || state === 'unavailable' || state === 'closed') return;
    state = 'unavailable';
    stopTimer();
    onEvent({ type: 'unavailable' });
  }

  function stopTimer() {
    if (connectTimer !== null) {
      clearTimeoutFn(connectTimer);
      connectTimer = null;
    }
    if (retryTimer !== null) {
      clearTimeoutFn(retryTimer);
      retryTimer = null;
    }
  }

  function wireChannel(ch) {
    channel = ch;
    ch.onopen = () => {
      state = 'open';
      stopTimer();
      onEvent({ type: 'open' });
    };
    ch.onmessage = (ev) => {
      const frame = decodeChatFrame(typeof ev === 'string' ? ev : ev && ev.data);
      if (!frame) return;
      if (frame.kind === 'taunt') {
        // Receiver-side taunt containment: a peer hammering
        // codes hears its own log fill, but OUR speakers stay
        // quiet inside the cooldown — the log line still shows
        // (it is the chat record), the sound does not repeat.
        const at = now();
        const audible = at - lastPeerTauntAt >= TAUNT_COOLDOWN_MS;
        if (audible) lastPeerTauntAt = at;
        onMessage({ ...frame, audible });
        return;
      }
      onMessage(frame);
    };
    ch.onclose = () => {
      if (state === 'open') {
        state = 'closed';
        onEvent({ type: 'closed' });
      } else {
        failClosed();
      }
    };
  }

  // The two links start on two machines at only roughly the
  // same moment (each side's 'go'): the guest's link may not
  // exist yet when the first offer crosses. A lost offer must
  // not doom the channel, so the host re-offers a few times
  // while connecting and unanswered — the guest answering any
  // of them ends the retries. ICE that arrives before the
  // remote description is queued and applied after it lands.
  async function sendOffer() {
    try {
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      sendSignal({ type: 'rtc', kind: 'offer', sdp: pc.localDescription.sdp });
    } catch {
      failClosed();
      return;
    }
    if (!answerSeen && offerRetries < maxOfferRetries && state === 'connecting') {
      retryTimer = setTimeoutFn(() => {
        offerRetries++;
        if (state === 'connecting' && !answerSeen) sendOffer();
      }, offerRetryMs);
    }
  }

  async function flushIce() {
    while (pendingIce.length && pc) {
      const c = pendingIce.shift();
      try { await pc.addIceCandidate(c); } catch { /* stale candidate: harmless */ }
    }
  }

  async function start() {
    if (state === 'connecting' || state === 'open') return;
    if (!factory) {
      failClosed();
      return;
    }
    state = 'connecting';
    stopTimer();
    connectTimer = setTimeoutFn(failClosed, RTC_CONNECT_TIMEOUT_MS);
    try {
      pc = new factory(config);
    } catch {
      failClosed();
      return;
    }
    pc.onicecandidate = (ev) => {
      const c = ev && ev.candidate;
      if (c && sendSignal) {
        sendSignal({
          type: 'rtc', kind: 'ice',
          candidate: c.candidate || '',
          sdpMid: c.sdpMid ?? null,
          sdpMLineIndex: c.sdpMLineIndex ?? null,
        });
      }
    };
    pc.oniceconnectionstatechange = () => {
      if (!pc) return;
      if (pc.iceConnectionState === 'failed' || pc.iceConnectionState === 'disconnected') {
        if (state === 'open') {
          state = 'closed';
          onEvent({ type: 'closed' });
        } else {
          failClosed();
        }
      }
    };
    if (isHost) {
      let ch;
      try {
        ch = pc.createDataChannel('chat', { negotiated: false });
      } catch {
        failClosed();
        return;
      }
      wireChannel(ch);
      sendOffer();
    } else {
      pc.ondatachannel = (ev) => {
        if (ev && ev.channel && ev.channel.label === 'chat') wireChannel(ev.channel);
      };
    }
  }

  /// Feed one inbound 'rtc' envelope from the room socket.
  async function handleSignal(msg) {
    if (!isSignalMessage(msg) || !pc) return;
    try {
      if (msg.kind === 'ice') {
        const c = {
          candidate: msg.candidate,
          sdpMid: msg.sdpMid,
          sdpMLineIndex: msg.sdpMLineIndex,
        };
        if (remoteSet) await pc.addIceCandidate(c);
        else pendingIce.push(c);
      } else if (msg.kind === 'offer' && !isHost) {
        await pc.setRemoteDescription({ type: 'offer', sdp: msg.sdp });
        remoteSet = true;
        await flushIce();
        const answer = await pc.createAnswer();
        await pc.setLocalDescription(answer);
        sendSignal({ type: 'rtc', kind: 'answer', sdp: pc.localDescription.sdp });
      } else if (msg.kind === 'answer' && isHost) {
        answerSeen = true;
        await pc.setRemoteDescription({ type: 'answer', sdp: msg.sdp });
        remoteSet = true;
        await flushIce();
      }
    } catch {
      failClosed();
    }
  }

  /// Send what the player typed. Returns the parsed frame when
  /// it went out (so the integrator can echo it into the local
  /// log and play a local taunt at once), or null when nothing
  /// was sent (empty, paced out, channel not open).
  function sendInput(raw) {
    const frame = parseChatInput(raw);
    if (!frame) return null;
    if (!channel || channel.readyState !== 'open') return null;
    const at = now();
    if (frame.kind === 'taunt') {
      if (at - lastTauntAt < TAUNT_COOLDOWN_MS) return null;
      lastTauntAt = at;
    } else {
      if (at - lastTextAt < CHAT_SEND_MIN_MS) return null;
      lastTextAt = at;
    }
    try {
      channel.send(JSON.stringify(
        frame.kind === 'taunt' ? { t: 'taunt', code: frame.code } : { t: 'text', text: frame.text },
      ));
    } catch {
      return null;
    }
    if (frame.kind === 'taunt') onLocalTaunt(frame.code);
    return frame;
  }

  function close() {
    stopTimer();
    try { if (channel) channel.close(); } catch { /* already gone */ }
    try { if (pc) pc.close(); } catch { /* already gone */ }
    channel = null;
    pc = null;
    if (state === 'open') onEvent({ type: 'closed' });
    state = 'closed';
  }

  return {
    start,
    handleSignal,
    sendInput,
    close,
    state: () => state,
    get open() { return state === 'open' && !!channel && channel.readyState === 'open'; },
  };
}
