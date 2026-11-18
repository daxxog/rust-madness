// nfm-web host glue — raw wasm, no bundler, no wasm-bindgen.
//
// W3 restyle: only the page chrome moved into index.html. Everything below —
// the `screen` canvas, the rAF/pixel blit, the window.__nfm probe hook, the
// AudioBus/ScriptProcessor bridge and the resume-on-gesture unlock — is the
// unchanged W1/W2 host. The archive-era look lives entirely in index.html.
//
// KEY TABLE (game codes): mirrors `crates/nfm-render/src/window.rs`'s WATCH
// (lines 43-56), the semantic source the shell's key handlers consume.
//   ArrowUp=1004  ArrowDown=1005  ArrowRight=1007  ArrowLeft=1006
//   Space=32  Enter=10  Escape=27
//   KeyA=97  KeyM=109  KeyN=110  KeyV=118  KeyP=112
// The wasm side turns Enter into a click on the fase-111 clicknow panel,
// because that gate reads only the mouse latch (shell.rs:219-233).

const KEYS = {
  ArrowUp: 1004, ArrowDown: 1005, ArrowRight: 1007, ArrowLeft: 1006,
  Space: 32, Enter: 10, Escape: 27,
  KeyA: 97, KeyM: 109, KeyN: 110, KeyV: 118, KeyP: 112,
};

const W = 550, H = 400;

let mem; // wasm memory, assigned after instantiate (js_log reads it too)

// Web audio state. Created at boot but SUSPENDED: browser autoplay policy
// requires a user gesture before an AudioContext may render. The gesture
// that dismisses the fase-111 gate (Enter / click) is the modern echo of
// the applet's click-to-start, and resumes the context there.
let audioCtx = null;
let scriptProcessor = null;
const audioStats = { callbacks: 0, frames: 0, nonzero: 0, peak: 0 };

function resumeAudio() {
  if (audioCtx && audioCtx.state !== 'running') {
    audioCtx.resume().then(
      () => console.log('[nfm] audio context running'),
      (err) => console.error('[nfm] audio resume failed', err),
    );
  }
}
// The archaic "applet loading..." line is honest chrome: it is removed on
// the first actual blit, and on a load failure its text becomes the error so
// a dead page never looks like a still-loading one.
function clearLoading() {
  const el = document.getElementById('loading');
  if (el) el.remove();
}
function failLoading(err) {
  const el = document.getElementById('loading');
  if (!el) return;
  el.textContent = 'load failed: ' + (err && err.message ? err.message : String(err));
  el.style.color = '#ff6666';
}

const imports = {
  env: {
    js_now_ms: () => performance.now(),
    js_log: (ptr, len) => {
      if (!mem) return;
      const bytes = new Uint8Array(mem.buffer, ptr, len);
      console.log('[nfm] ' + new TextDecoder().decode(bytes));
    },
    // The browser cookie jar's stand-in: one localStorage entry holding the
    // whole `key=value` jar text. TextEncoder/TextDecoder bridge it to the
    // wasm bytes nfm-core's cfg'd load()/save() pass. Any storage failure
    // (private mode, quota, disabled API) degrades to session-only — never a
    // trap into wasm, never a console error.
    js_ls_get: (buf, cap) => {
      if (!mem) return 0;
      try {
        const raw = localStorage.getItem('nfm-cookies');
        if (raw === null) return 0;
        const bytes = new TextEncoder().encode(raw);
        const n = Math.min(bytes.length, cap);
        if (n > 0) new Uint8Array(mem.buffer, buf, n).set(bytes.subarray(0, n));
        return n;
      } catch (_) {
        return 0;
      }
    },
    js_ls_set: (ptr, len) => {
      if (!mem) return;
      try {
        const text = new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len));
        localStorage.setItem('nfm-cookies', text);
      } catch (_) {
        /* session-only this run */
      }
    },
  },
};

(async () => {
  // The build produces nfm_web.wasm; the release pipeline renames it to
  // rust-madness.wasm when packaging/deploying the site, which is what we fetch.
  const response = await fetch('rust-madness.wasm');
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), imports);
  const ex = instance.exports;
  mem = ex.memory;

  ex.nfm_init();

  // Mixer hardware substitute: a ScriptProcessorNode driven on the main
  // thread, so its callback calls straight INTO the wasm instance — no
  // ring buffer, no second memory, and no COOP/COEP headers that GitHub
  // Pages cannot set. ScriptProcessor is deprecated but universally
  // supported; an AudioWorklet would need SharedArrayBuffer + a separate
  // module. The bus honours ANY output rate (AudioBus::pull resamples each
  // voice by voice.rate / rate), so we pull at the context's native rate
  // and skip the JS upsampling pass entirely; the only JS mixing work is
  // the i16→f32 conversion and the mono→channels duplication.
  const AudioCtx = window.AudioContext || window.webkitAudioContext;
  audioCtx = new AudioCtx();
  ex.nfm_audio_set_rate(audioCtx.sampleRate);
  const audioChannels = audioCtx.destination.channelCount;
  scriptProcessor = audioCtx.createScriptProcessor(2048, 0, audioChannels);
  const cap = ex.nfm_audio_cap();
  scriptProcessor.onaudioprocess = (e) => {
    const total = e.outputBuffer.length;
    const frames = Math.min(total, cap);
    // Fresh ptr each callback: a growing wasm memory detaches old views.
    const ptr = ex.nfm_audio_buf_ptr();
    ex.nfm_audio(ptr, frames);
    const mono = new Int16Array(mem.buffer, ptr, frames);
    let nz = 0, peak = 0;
    for (let i = 0; i < frames; i++) {
      const v = mono[i];
      if (v !== 0) nz++;
      const av = v < 0 ? -v : v;
      if (av > peak) peak = av;
    }
    audioStats.callbacks++;
    audioStats.frames += frames;
    audioStats.nonzero += nz;
    if (peak > audioStats.peak) audioStats.peak = peak;
    for (let ch = 0; ch < e.outputBuffer.numberOfChannels; ch++) {
      const dst = e.outputBuffer.getChannelData(ch);
      for (let i = 0; i < frames; i++) dst[i] = mono[i] / 32768;
      for (let i = frames; i < total; i++) dst[i] = 0;
    }
  };
  scriptProcessor.connect(audioCtx.destination);

  const canvas = document.getElementById('screen');
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  const image = ctx.createImageData(W, H);

  // Era-styled text control in the applet chrome. Click toggles fullscreen;
  // Esc exits natively. The aspect-preserving letterbox is pure CSS.
  const fsBtn = document.getElementById('fullscreen');
  if (fsBtn) {
    fsBtn.addEventListener('click', () => {
      if (document.fullscreenElement) {
        document.exitFullscreen().catch(() => {});
      } else {
        canvas.requestFullscreen().catch(() => {});
      }
    });
  }

  let painted = false;
  function blit() {
    const ptr = ex.nfm_frame_ptr();
    const len = ex.nfm_frame_len();
    // Fresh view every time: a growing wasm memory detaches the old buffer.
    const rgb = new Uint8Array(mem.buffer, ptr, len);
    const out = image.data;
    for (let i = 0, j = 0; i < len; i += 3, j += 4) {
      out[j] = rgb[i];
      out[j + 1] = rgb[i + 1];
      out[j + 2] = rgb[i + 2];
      out[j + 3] = 255;
    }
    ctx.putImageData(image, 0, 0);
    // The canvas is genuinely painted now: retire the loading chrome once.
    if (!painted) {
      painted = true;
      clearLoading();
    }
  }

  function frame() {
    ex.nfm_tick();
    blit();
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);

  window.addEventListener('keydown', (e) => {
    // First real gesture also satisfies the autoplay policy.
    resumeAudio();
    if (Object.prototype.hasOwnProperty.call(KEYS, e.code)) {
      ex.nfm_key(KEYS[e.code], 1);
      e.preventDefault();
    }
  });
  window.addEventListener('keyup', (e) => {
    if (Object.prototype.hasOwnProperty.call(KEYS, e.code)) {
      ex.nfm_key(KEYS[e.code], 0);
      e.preventDefault();
    }
  });
  canvas.addEventListener('mousedown', (e) => {
    resumeAudio();
    const r = canvas.getBoundingClientRect();
    const x = Math.round((e.clientX - r.left) * W / r.width);
    const y = Math.round((e.clientY - r.top) * H / r.height);
    ex.nfm_mousedown(x, y);
  });

  // Any pointer gesture is a valid autoplay unlock, not just canvas clicks.
  window.addEventListener('pointerdown', resumeAudio);

  // Probe hook: lets the browser gate dispatch keys/press and read pixels
  // without depending on real focus. Not used by the game itself.
  window.__nfm = {
    ex,
    canvas,
    KEYS,
    key(code, down) { ex.nfm_key(code, down); },
    click(x, y) { ex.nfm_mousedown(x, y); },
    audio: {
      get state() { return audioCtx ? audioCtx.state : 'none'; },
      get sampleRate() { return audioCtx ? audioCtx.sampleRate : 0; },
      get channels() { return audioChannels; },
      get pulled() { return Number(ex.nfm_audio_pulled()); },
      get callbacks() { return audioStats.callbacks; },
      get frames() { return audioStats.frames; },
      get nonzero() { return audioStats.nonzero; },
      get peak() { return audioStats.peak; },
      resume() { resumeAudio(); },
    },
    nonblack() {
      // rAF can stall in headless; blit the current frame first so this
      // probe hook always reads the latest wasm canvas.
      blit();
      const d = ctx.getImageData(0, 0, W, H).data;
      let n = 0;
      for (let i = 0; i < d.length; i += 4) {
        if (d[i] || d[i + 1] || d[i + 2]) n++;
      }
      return n;
    },
  };
  console.log('[nfm] probe ready');
})().catch((err) => {
  console.error('[nfm] boot error', err);
  failLoading(err);
});
