// Unit tests for P2P chat (play/chat.js): the taunt map, input
// parsing, frame validation, and the full link state machine
// over a fake RTC pair — including the privacy invariant this
// feature exists for: chat content NEVER appears in a signaling
// frame (the only thing the relay ever sees), and a link that
// cannot connect fails closed instead of falling back.
// Run from web-preview/play/:  node --test chat.test.mjs

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  TAUNTS, TAUNT_CODES, CHAT_TEXT_MAX, TAUNT_COOLDOWN_MS,
  parseChatInput, decodeChatFrame, isSignalMessage, createChatLink,
} from './chat.js';

// ---------------------------------------------------------------- taunts

test('every mapped taunt code has a phrase, and the classics are where a DE player expects them', () => {
  for (const code of TAUNT_CODES) {
    assert.equal(typeof TAUNTS[code], 'string');
    assert.ok(TAUNTS[code].length > 0);
  }
  // DE numbering semantics (public documentation): 1 yes, 2 no,
  // 3-6 the four resources, 7 the attack call, 11 the laugh,
  // 12/13 resupply asks, 14 begin, 30 the chant.
  assert.deepEqual(TAUNT_CODES, [1, 2, 3, 4, 5, 6, 7, 11, 12, 13, 14, 30]);
});

test('parseChatInput: mapped bare numbers are taunts, everything else is text', () => {
  assert.deepEqual(parseChatInput('1'), { kind: 'taunt', code: 1 });
  assert.deepEqual(parseChatInput(' 30 '), { kind: 'taunt', code: 30 });
  assert.deepEqual(parseChatInput('42'), { kind: 'text', text: '42' }); // unmapped
  assert.deepEqual(parseChatInput('7 come help'), { kind: 'text', text: '7 come help' });
  assert.deepEqual(parseChatInput('gl hf'), { kind: 'text', text: 'gl hf' });
  assert.equal(parseChatInput('   '), null);
  assert.equal(parseChatInput(''), null);
  assert.equal(parseChatInput(7), null);
  const long = parseChatInput('x'.repeat(500));
  assert.equal(long.text.length, CHAT_TEXT_MAX);
});

test('decodeChatFrame validates shape, kinds and caps', () => {
  assert.deepEqual(decodeChatFrame('{"t":"text","text":"hi"}'), { kind: 'text', text: 'hi' });
  assert.deepEqual(decodeChatFrame('{"t":"taunt","code":11}'), { kind: 'taunt', code: 11 });
  assert.equal(decodeChatFrame('{"t":"taunt","code":42}'), null); // unmapped code
  assert.equal(decodeChatFrame('{"t":"taunt","code":"11"}'), null); // wrong type
  assert.equal(decodeChatFrame('{"t":"text","text":"  "}'), null);
  assert.equal(decodeChatFrame('{"t":"nope"}'), null);
  assert.equal(decodeChatFrame('not json'), null);
  assert.equal(decodeChatFrame('{"t":"text","text":"a\\tb"}').text, 'ab'); // control chars stripped
  assert.equal(
    decodeChatFrame(JSON.stringify({ t: 'text', text: 'y'.repeat(400) })).text.length,
    CHAT_TEXT_MAX,
  );
});

test('isSignalMessage accepts only the three signaling kinds', () => {
  assert.equal(isSignalMessage({ type: 'rtc', kind: 'offer' }), true);
  assert.equal(isSignalMessage({ type: 'rtc', kind: 'ice' }), true);
  assert.equal(isSignalMessage({ type: 'rtc', kind: 'text' }), false);
  assert.equal(isSignalMessage({ type: 'bundle', kind: 'offer' }), false);
  assert.equal(isSignalMessage(null), false);
});

// ------------------------------------------------------------ fake RTC
//
// A mated pair of fake peer connections: the second PC created
// is the guest's; when it takes the offer as its remote
// description, the two channel ends are joined and both open.

function makeRtcPair() {
  const pcs = [];
  class FakeChannel {
    constructor(label) {
      this.label = label;
      this.readyState = 'connecting';
      this.peer = null;
      this.onopen = null;
      this.onmessage = null;
      this.onclose = null;
    }
    send(data) {
      const peer = this.peer;
      queueMicrotask(() => { if (peer && peer.onmessage) peer.onmessage({ data }); });
    }
    close() {
      this.readyState = 'closed';
      if (this.onclose) this.onclose();
      const peer = this.peer;
      if (peer && peer.readyState !== 'closed') {
        peer.readyState = 'closed';
        queueMicrotask(() => { if (peer.onclose) peer.onclose(); });
      }
    }
  }
  class FakePC {
    constructor() {
      this.localDescription = null;
      this.onicecandidate = null;
      this.ondatachannel = null;
      this.oniceconnectionstatechange = null;
      this.iceConnectionState = 'new';
      pcs.push(this);
    }
    createDataChannel(label) {
      this.hostChannel = new FakeChannel(label);
      return this.hostChannel;
    }
    async createOffer() { return { type: 'offer', sdp: 'fake-offer-sdp' }; }
    async createAnswer() { return { type: 'answer', sdp: 'fake-answer-sdp' }; }
    async setLocalDescription(d) { this.localDescription = d; }
    async setRemoteDescription(d) {
      this.remoteDescription = d;
      if (d.type === 'offer' && pcs[0] && pcs[0].hostChannel) {
        const guestEnd = new FakeChannel('chat');
        const hostEnd = pcs[0].hostChannel;
        hostEnd.peer = guestEnd;
        guestEnd.peer = hostEnd;
        this.guestChannel = guestEnd;
        queueMicrotask(() => {
          if (this.ondatachannel) this.ondatachannel({ channel: guestEnd });
          for (const end of [hostEnd, guestEnd]) {
            end.readyState = 'open';
            if (end.onopen) end.onopen();
          }
        });
      }
    }
    async addIceCandidate(c) { this.lastCandidate = c; }
    close() { this.iceConnectionState = 'closed'; }
  }
  return { factory: FakePC, pcs };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

/// Two links joined by a recording "relay": every signaling
/// frame is captured (this is everything a real relay could
/// ever see) and delivered to the other side.
async function linkedPair() {
  const { factory, pcs } = makeRtcPair();
  const signalLog = [];
  let clock = 1_000_000;
  const now = () => clock;
  const hostIn = [];
  const guestIn = [];
  const hostTaunts = [];
  const guestTaunts = [];
  const hostEvents = [];
  const guestEvents = [];
  let host;
  let guest;
  const deliver = (to, msg) => queueMicrotask(() => to.handleSignal(msg));
  host = createChatLink({
    isHost: true,
    rtcFactory: factory,
    now,
    offerRetryMs: 5,
    sendSignal: (m) => { signalLog.push(m); deliver(guest, m); },
    onMessage: (f) => hostIn.push(f),
    onLocalTaunt: (c) => hostTaunts.push(c),
    onEvent: (e) => hostEvents.push(e),
  });
  guest = createChatLink({
    isHost: false,
    rtcFactory: factory,
    now,
    sendSignal: (m) => { signalLog.push(m); deliver(host, m); },
    onMessage: (f) => guestIn.push(f),
    onLocalTaunt: (c) => guestTaunts.push(c),
    onEvent: (e) => guestEvents.push(e),
  });
  await host.start();
  await guest.start();
  // The host's first offer crossed before the guest's link
  // existed and was lost — exactly the real race (the two
  // sides' 'go' moments are only roughly simultaneous). The
  // host's offer retries are what land the channel.
  await new Promise((r) => setTimeout(r, 40));
  await tick(); await tick();
  return {
    host, guest, signalLog, hostIn, guestIn, hostTaunts, guestTaunts,
    hostEvents, guestEvents, pcs,
    advance: (ms) => { clock += ms; },
  };
}

test('two links connect over signaling and exchange text + taunts — content never in a signal frame', async () => {
  const p = await linkedPair();
  assert.equal(p.host.open, true);
  assert.equal(p.guest.open, true);
  assert.ok(p.signalLog.some((m) => m.kind === 'offer'));
  assert.ok(p.signalLog.some((m) => m.kind === 'answer'));

  const secret = 'attack at dawn, bring wood';
  const sent = p.host.sendInput(secret);
  assert.deepEqual(sent, { kind: 'text', text: secret });
  await tick();
  assert.deepEqual(p.guestIn, [{ kind: 'text', text: secret }]);

  const taunt = p.guest.sendInput('7');
  assert.deepEqual(taunt, { kind: 'taunt', code: 7 });
  await tick();
  assert.deepEqual(p.hostIn, [{ kind: 'taunt', code: 7, audible: true }]);
  assert.deepEqual(p.guestTaunts, [7]); // the sender's own local playback hook

  // THE privacy invariant: everything the relay carried is
  // signaling; no chat text, no phrase, no code-as-content.
  for (const frame of p.signalLog) {
    const wire = JSON.stringify(frame);
    assert.ok(!wire.includes('attack at dawn'), 'chat text leaked into signaling');
    assert.ok(!wire.includes('under attack'), 'taunt phrase leaked into signaling');
    assert.equal(frame.type, 'rtc');
  }
});

test('taunt cooldown: the sender cannot machine-gun, and a spamming peer goes quiet (not invisible)', async () => {
  const p = await linkedPair();
  assert.ok(p.host.sendInput('11'));
  assert.equal(p.host.sendInput('11'), null); // own cooldown, instantly
  await tick();
  assert.equal(p.guestIn.filter((f) => f.kind === 'taunt').length, 1);

  // The peer bypasses its own client and hammers raw frames:
  // the receiver logs both but only hears the first.
  const guestPc = p.pcs[1];
  const raw = (obj) => guestPc.guestChannel.send(JSON.stringify(obj));
  raw({ t: 'taunt', code: 30 });
  await tick();
  raw({ t: 'taunt', code: 30 });
  await tick();
  const heard = p.hostIn.filter((f) => f.kind === 'taunt' && f.code === 30);
  assert.equal(heard.length, 2);
  assert.deepEqual(heard.map((f) => f.audible), [true, false]);

  // After the cooldown the same code is audible again.
  p.advance(TAUNT_COOLDOWN_MS + 1);
  raw({ t: 'taunt', code: 30 });
  await tick();
  const heard2 = p.hostIn.filter((f) => f.kind === 'taunt' && f.code === 30);
  assert.equal(heard2.at(-1).audible, true);

  // And the sender's own cooldown lifts with the clock too.
  assert.ok(p.host.sendInput('11'));
});

test('malformed data-channel frames are dropped, never fatal', async () => {
  const p = await linkedPair();
  const guestPc = p.pcs[1];
  guestPc.guestChannel.send('garbage{');
  guestPc.guestChannel.send('{"t":"taunt","code":99}');
  guestPc.guestChannel.send('{"t":"evil","text":"x"}');
  await tick();
  assert.deepEqual(p.hostIn, []);
  assert.ok(p.guest.sendInput('still alive') !== null);
  await tick();
  assert.deepEqual(p.hostIn, [{ kind: 'text', text: 'still alive' }]);
});

test('a link that cannot connect fails closed: unavailable event, sends refused, no fallback', async () => {
  const events = [];
  class DeadPC {
    constructor() { this.onicecandidate = null; this.oniceconnectionstatechange = null; }
    createDataChannel() {
      return { readyState: 'connecting', onopen: null, onmessage: null, onclose: null, send() {}, close() {} };
    }
    async createOffer() { return { type: 'offer', sdp: 'x' }; }
    async setLocalDescription() {}
    async setRemoteDescription() {}
    async addIceCandidate() {}
    close() {}
  }
  let timerFn = null;
  const link = createChatLink({
    isHost: true,
    rtcFactory: DeadPC,
    sendSignal: () => {},
    onEvent: (e) => events.push(e),
    setTimeoutFn: (fn) => { timerFn = fn; return 1; },
    clearTimeoutFn: () => {},
  });
  await link.start();
  timerFn(); // the connect timeout fires
  assert.deepEqual(events, [{ type: 'unavailable' }]);
  assert.equal(link.state(), 'unavailable');
  assert.equal(link.sendInput('hello?'), null);
  assert.equal(link.sendInput('1'), null);
});

test('ICE candidates from the pc are forwarded as signaling (and only signaling)', async () => {
  const { factory, pcs } = makeRtcPair();
  const signals = [];
  const link = createChatLink({
    isHost: true,
    rtcFactory: factory,
    sendSignal: (m) => signals.push(m),
    onMessage: () => {},
  });
  await link.start();
  pcs[0].onicecandidate({
    candidate: { candidate: 'candidate:1 1 udp 1 192.0.2.1 9 typ host', sdpMid: '0', sdpMLineIndex: 0 },
  });
  const ice = signals.find((m) => m.kind === 'ice');
  assert.ok(ice);
  assert.equal(ice.type, 'rtc');
  assert.ok(ice.candidate.startsWith('candidate:'));
});
