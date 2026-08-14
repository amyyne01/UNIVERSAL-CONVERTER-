/* Splash text layer. Deliberately dependency-free vanilla JS, loaded with `defer`
   BEFORE the app bundle: it has to animate from the first frame, and at that point
   React, framer-motion and the stylesheet do not exist yet. Production CSP is
   script-src 'self', so this cannot be inline either.
   Division of labour: this file owns the WORDS (wordmark reveal + the looping
   status line). src/lib/splash.ts owns the PROGRESS BAR and the dismissal, and
   calls window.__ahgSplash.stop() on its way out. */
(function () {
  'use strict';

  /* Max six words each. They are jokes, so they never claim a boot step that did
     not happen — the bar is where the honest progress lives. */
  var MESSAGES = [
    'Soundchecking the interface',
    'Quantizing the layout grid',
    'Warming up the valves',
    'Adding more cowbell',
    'Waiting for the drop',
    'Clearing the samples',
    'Rolling tape',
    'Auto-tuning the sidebar',
    'Skipping the drum solo',
    'Dropping the needle',
    'Polishing the gold master',
    'Chasing a sync licence',
    'Reading the liner notes',
    'Cueing the next track',
    'Feeding the algorithm',
    'Bouncing the final mix',
    'Sweeping the low end',
    'Rehearsing the transitions',
    'Sequencing the components',
    'Un-clipping the master bus',
  ];

  var GLYPHS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#*/\\<>[]{}=~';

  var FLIP_MS = 320;      // one character's flip
  var FLIP_STAGGER = 22;  // per-character offset
  var HOLD_MS = 1500;     // how long a finished line reads before the next swap
  /* The wordmark reveal is deliberately unhurried — it is the first thing anyone
     sees of the app. ~2.2s end to end for 22 characters. */
  var SCRAMBLE_CHAR_MS = 420;
  var SCRAMBLE_STAGGER = 80;
  /* Churning a new glyph every frame reads as noise; ~16/s reads as a machine
     working through it. */
  var GLYPH_SWAP_MS = 60;

  /* Reduced motion gets a gentler ALTERNATIVE, never silence. The preference is
     about vestibular motion — 3D character flips and glyph churn are exactly what
     it means to avoid, but a crossfade is the standard substitute. Switching the
     whole splash off is what makes it look broken to anyone who has Windows
     "Show animations" turned off, which is a very common default. */
  var reduced =
    typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

  var stopped = false;
  var timers = [];
  function later(fn, ms) {
    var id = setTimeout(fn, ms);
    timers.push(id);
    return id;
  }
  function clearTimers() {
    for (var i = 0; i < timers.length; i++) clearTimeout(timers[i]);
    timers = [];
  }

  /* ── Wordmark: a one-shot scramble reveal, settling left to right ───────────
     Scrambling glyphs are painted in the accent so the resolve reads as the
     signal locking on, matching what the trace below is doing. */
  function scrambleWordmark(root) {
    var targets = root.querySelectorAll('.sp-word b, .sp-word span');
    var cells = [];

    for (var t = 0; t < targets.length; t++) {
      var el = targets[t];
      var text = el.textContent;
      el.textContent = '';
      for (var i = 0; i < text.length; i++) {
        var span = document.createElement('span');
        span.className = 'sp-ch';
        if (text[i] === ' ') {
          span.textContent = ' ';
          el.appendChild(span);
          continue;
        }
        span.textContent = reduced ? text[i] : GLYPHS[(Math.random() * GLYPHS.length) | 0];
        if (reduced) {
          span.style.opacity = '0';
          span.style.transition = 'opacity 260ms ease-out ' + cells.length * 26 + 'ms';
        } else {
          span.className = 'sp-ch sp-scramble';
        }
        el.appendChild(span);
        cells.push({ el: span, ch: text[i] });
      }
    }

    if (reduced) {
      // One frame with opacity 0 applied, then let the staggered fade run.
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          for (var i = 0; i < cells.length; i++) cells[i].el.style.opacity = '1';
        });
      });
      return;
    }

    var start = 0;
    var lastSwap = 0;
    function frame(now) {
      if (stopped) return;
      if (!start) start = now;
      var elapsed = now - start;
      var churn = now - lastSwap >= GLYPH_SWAP_MS;
      if (churn) lastSwap = now;
      var settled = 0;
      for (var i = 0; i < cells.length; i++) {
        var c = cells[i];
        if (elapsed >= i * SCRAMBLE_STAGGER + SCRAMBLE_CHAR_MS) {
          if (c.el.textContent !== c.ch) {
            c.el.textContent = c.ch;
            c.el.className = 'sp-ch';
          }
          settled++;
        } else if (churn && elapsed >= i * SCRAMBLE_STAGGER) {
          c.el.textContent = GLYPHS[(Math.random() * GLYPHS.length) | 0];
        }
      }
      if (settled < cells.length) requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }

  /* ── Status line: character flip between messages ──────────────────────────
     Same shape as the reference: the outgoing line rotates away around its top
     edge while the incoming one arrives from below, staggered per character. */
  function buildChars(host, text, incoming) {
    host.textContent = '';
    var chars = [];
    for (var i = 0; i < text.length; i++) {
      var span = document.createElement('span');
      span.className = 'sp-ch';
      span.textContent = text[i] === ' ' ? ' ' : text[i];
      if (incoming && !reduced) {
        span.style.transform = 'rotateX(-90deg)';
        span.style.opacity = '0';
        span.style.transformOrigin = 'center bottom';
      }
      host.appendChild(span);
      chars.push(span);
    }
    return chars;
  }

  /** Fisher-Yates: a shuffled walk shows all twenty before any repeats, which
   *  pure random does not — it would happily show the same line three times. */
  function shuffled(list) {
    var a = list.slice();
    for (var i = a.length - 1; i > 0; i--) {
      var j = (Math.random() * (i + 1)) | 0;
      var tmp = a[i];
      a[i] = a[j];
      a[j] = tmp;
    }
    return a;
  }

  function startMessages(host) {
    var order = shuffled(MESSAGES);
    var index = 0;
    var current = buildChars(host, order[index], false);

    function stagger(len) {
      // Keep a long line's total sweep close to a short one's.
      return Math.min(FLIP_STAGGER, 420 / Math.max(len, 1));
    }

    function swap() {
      if (stopped) return;
      index = (index + 1) % order.length;
      if (index === 0) order = shuffled(MESSAGES); // reshuffle each full pass
      var next = order[index];

      if (reduced) {
        host.style.transition = 'opacity 240ms ease-out';
        host.style.opacity = '0';
        later(function () {
          if (stopped) return;
          current = buildChars(host, next, false);
          host.style.opacity = '1';
          later(swap, HOLD_MS + 240);
        }, 240);
        return;
      }

      var st = stagger(current.length);
      for (var i = 0; i < current.length; i++) {
        var el = current[i];
        el.style.transformOrigin = 'center top';
        el.style.transitionDelay = i * st + 'ms';
        el.style.transform = 'rotateX(90deg)';
        el.style.opacity = '0';
      }

      var outTotal = FLIP_MS + current.length * st;
      // Overlap the arrival, as the reference does, so the line never reads empty.
      later(function () {
        if (stopped) return;
        var chars = buildChars(host, next, true);
        var st2 = stagger(chars.length);
        // One frame with the start transform applied, then transition to rest.
        requestAnimationFrame(function () {
          requestAnimationFrame(function () {
            for (var i = 0; i < chars.length; i++) {
              chars[i].style.transitionDelay = i * st2 + 'ms';
              chars[i].style.transform = 'rotateX(0deg)';
              chars[i].style.opacity = '1';
            }
          });
        });
        current = chars;
        later(swap, FLIP_MS + chars.length * st2 + HOLD_MS);
      }, outTotal * 0.7);
    }

    later(swap, HOLD_MS);
  }

  function init() {
    var splash = document.getElementById('splash');
    var status = document.getElementById('splash-status');
    if (!splash || !status) return;
    scrambleWordmark(splash);
    startMessages(status);
  }

  window.__ahgSplash = {
    /** Called by splash.ts as it dismisses, so nothing animates a removed node. */
    stop: function () {
      stopped = true;
      clearTimers();
    },
  };

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
