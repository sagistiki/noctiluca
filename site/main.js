// The page's motion and the live parts: reveal on scroll, the progress line,
// counters, the light that follows the pointer on cards, a gentle parallax,
// the looping gallery, copy buttons, and download buttons that point at the
// latest release's files.
(() => {
  const REPO = 'sagistiki/noctiluca';
  const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // ---- reveal on scroll ----
  const reveal = document.querySelectorAll('[data-reveal]');
  if ('IntersectionObserver' in window && !still) {
    const io = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); }
      }
    }, { threshold: 0.12, rootMargin: '0px 0px -40px 0px' });
    reveal.forEach((el) => io.observe(el));
  } else {
    reveal.forEach((el) => el.classList.add('in'));
  }

  // ---- counters ----
  const counters = document.querySelectorAll('[data-count]');
  const countUp = (el) => {
    const target = +el.dataset.count;
    if (still) { el.textContent = target; return; }
    const t0 = performance.now(), dur = 1400;
    const step = (now) => {
      const k = Math.min(1, (now - t0) / dur);
      el.textContent = Math.round(target * (1 - Math.pow(1 - k, 3)));
      if (k < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  };
  if ('IntersectionObserver' in window) {
    const co = new IntersectionObserver((entries) => {
      for (const e of entries) if (e.isIntersecting) { countUp(e.target); co.unobserve(e.target); }
    }, { threshold: 0.6 });
    counters.forEach((el) => { el.textContent = '0'; co.observe(el); });
  }

  // ---- navigation, progress, parallax: one pass per frame while scrolling ----
  const nav = document.querySelector('.nav');
  const bar = document.querySelector('.progress span');
  const layers = [...document.querySelectorAll('[data-parallax]')];
  let ticking = false;
  const onScroll = () => {
    const y = window.scrollY;
    const max = document.documentElement.scrollHeight - innerHeight;
    nav.classList.toggle('scrolled', y > 20);
    bar.style.setProperty('--p', max > 0 ? (y / max).toFixed(4) : 0);
    if (!still) {
      for (const el of layers) {
        const r = el.getBoundingClientRect();
        const off = (r.top + r.height / 2 - innerHeight / 2) * parseFloat(el.dataset.parallax);
        el.style.translate = `0 ${off.toFixed(1)}px`;
      }
    }
    ticking = false;
  };
  window.addEventListener('scroll', () => { if (!ticking) { ticking = true; requestAnimationFrame(onScroll); } }, { passive: true });
  onScroll();

  // ---- the light that follows the pointer, and a slight tilt, on cards ----
  if (!still && window.matchMedia('(hover: hover)').matches) {
    document.querySelectorAll('.card, .tile').forEach((card) => {
      card.addEventListener('pointermove', (e) => {
        const r = card.getBoundingClientRect();
        const x = (e.clientX - r.left) / r.width, y = (e.clientY - r.top) / r.height;
        card.style.setProperty('--mx', `${x * 100}%`);
        card.style.setProperty('--my', `${y * 100}%`);
        card.style.setProperty('--rx', `${(0.5 - y) * 6}deg`);
        card.style.setProperty('--ry', `${(x - 0.5) * 8}deg`);
      });
      card.addEventListener('pointerleave', () => {
        card.style.setProperty('--rx', '0deg');
        card.style.setProperty('--ry', '0deg');
      });
    });
  }

  // ---- the gallery loops seamlessly: its pictures twice over ----
  const track = document.querySelector('.track');
  if (track) {
    [...track.children].forEach((img) => {
      const copy = img.cloneNode(true);
      copy.alt = '';
      copy.setAttribute('aria-hidden', 'true');
      track.appendChild(copy);
    });
  }

  // ---- copy buttons ----
  document.querySelectorAll('[data-copy]').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const text = document.querySelector(btn.dataset.copy).textContent;
      try { await navigator.clipboard.writeText(text); btn.textContent = 'Copied'; }
      catch { const r = document.createRange(); r.selectNodeContents(document.querySelector(btn.dataset.copy)); getSelection().removeAllRanges(); getSelection().addRange(r); btn.textContent = 'Selected'; }
      setTimeout(() => (btn.textContent = 'Copy'), 1800);
    });
  });

  // ---- the latest release: version, date, and each kind of download ----
  // Buttons start on the releases page; once GitHub answers they point
  // straight at the file. If it does not answer, they still work.
  const kinds = {
    macos: { match: (n) => /macos.*\.zip$/i.test(n), label: (a) => `Apple silicon · ${mb(a.size)}` },
    apk: { match: (n) => /\.apk$/i.test(n), label: (a) => `Android TV · ${mb(a.size)}` },
  };
  const mb = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`;
  const date = (iso) => new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' });
  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: 'application/vnd.github+json' } })
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((rel) => {
      const version = rel.tag_name.replace(/^v/, '');
      document.querySelectorAll('[data-version]').forEach((el) => (el.textContent = version));
      document.querySelectorAll('[data-release]').forEach((el) => (el.textContent = `Version ${version} · released ${date(rel.published_at)}`));
      document.querySelectorAll('[data-release-date]').forEach((el) => (el.textContent = `Version ${version}, released ${date(rel.published_at)}. Free, for Macs with Apple silicon on macOS 13 or later.`));
      for (const [kind, k] of Object.entries(kinds)) {
        const asset = rel.assets.find((a) => k.match(a.name));
        document.querySelectorAll(`[data-asset="${kind}"]`).forEach((btn) => {
          btn.href = asset ? asset.browser_download_url : rel.html_url;
        });
        if (asset) document.querySelectorAll(`[data-asset-meta="${kind}"]`).forEach((el) => (el.textContent = k.label(asset)));
      }
    })
    .catch(() => {
      document.querySelectorAll('[data-release]').forEach((el) => (el.textContent = 'Latest release on GitHub'));
    });

  // ---- stars on the GitHub button ----
  fetch(`https://api.github.com/repos/${REPO}`)
    .then((r) => (r.ok ? r.json() : Promise.reject()))
    .then((repo) => {
      const el = document.querySelector('[data-stars]');
      if (el && repo.stargazers_count > 0) { el.textContent = `★ ${repo.stargazers_count}`; el.hidden = false; }
    })
    .catch(() => {});
})();
