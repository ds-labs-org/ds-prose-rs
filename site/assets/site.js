// Behaviour for the site shell and the stamped demos. Progressive: without
// this file the menu list is visible and the theme follows the system.
(function () {
  'use strict';
  var KEY = 'ds-prose-theme';
  var root = document.documentElement;

  function storedTheme() {
    try {
      var t = localStorage.getItem(KEY);
      return t === 'light' || t === 'dark' ? t : 'auto';
    } catch (e) {
      return 'auto';
    }
  }
  function storeTheme(t) {
    try {
      if (t === 'auto') localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, t);
    } catch (e) {
      /* storage unavailable: the choice lasts for this page only */
    }
  }

  function initTheme() {
    var btn = document.querySelector('.theme-toggle');
    if (!btn) return;
    var current = root.getAttribute('data-theme') || 'auto';
    function show() {
      btn.textContent = 'Theme: ' + current;
      btn.setAttribute('aria-pressed', current === 'auto' ? 'false' : 'true');
    }
    show();
    btn.addEventListener('click', function () {
      current = current === 'auto' ? 'light' : current === 'light' ? 'dark' : 'auto';
      if (current === 'auto') root.removeAttribute('data-theme');
      else root.setAttribute('data-theme', current);
      storeTheme(current);
      show();
    });
  }

  function initMenu() {
    var btn = document.querySelector('.menu-toggle');
    if (!btn) return;
    var links = document.getElementById(btn.getAttribute('aria-controls'));
    if (!links) return;
    var narrow = window.matchMedia('(max-width: 47.99rem)');
    function setOpen(open) {
      btn.setAttribute('aria-expanded', open ? 'true' : 'false');
      links.hidden = !open;
    }
    function sync() {
      if (narrow.matches) {
        btn.hidden = false;
        setOpen(false);
      } else {
        btn.hidden = true;
        btn.setAttribute('aria-expanded', 'false');
        links.hidden = false;
      }
    }
    btn.addEventListener('click', function () {
      setOpen(btn.getAttribute('aria-expanded') !== 'true');
    });
    document.addEventListener('keydown', function (e) {
      if (e.key === 'Escape' && narrow.matches && btn.getAttribute('aria-expanded') === 'true') {
        setOpen(false);
        btn.focus();
      }
    });
    if (narrow.addEventListener) narrow.addEventListener('change', sync);
    sync();
  }

  function initCopy() {
    var buttons = document.querySelectorAll('[data-copy]');
    if (!buttons.length) return;
    var status = document.createElement('div');
    status.className = 'sr-only';
    status.setAttribute('role', 'status');
    document.body.appendChild(status);

    function fallback(text) {
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.setAttribute('readonly', '');
      ta.className = 'sr-only';
      document.body.appendChild(ta);
      ta.select();
      var ok = false;
      try {
        ok = document.execCommand('copy');
      } catch (e) {
        ok = false;
      }
      document.body.removeChild(ta);
      return ok ? Promise.resolve() : Promise.reject(new Error('copy failed'));
    }

    Array.prototype.forEach.call(buttons, function (btn) {
      btn.addEventListener('click', function () {
        var code = btn.parentElement.querySelector('code');
        var text = code ? code.textContent : '';
        var done = function (msg) {
          btn.textContent = msg;
          status.textContent = msg;
          setTimeout(function () {
            btn.textContent = 'Copy';
          }, 2000);
        };
        var write =
          navigator.clipboard && navigator.clipboard.writeText
            ? navigator.clipboard.writeText(text).catch(function () {
                return fallback(text);
              })
            : fallback(text);
        write.then(
          function () {
            done('Copied');
          },
          function () {
            done('Copy failed');
          },
        );
      });
    });
  }

  function init() {
    initTheme();
    initMenu();
    initCopy();
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
