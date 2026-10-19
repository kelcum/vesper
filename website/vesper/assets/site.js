(() => {
  const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;

  const items = document.querySelectorAll('.reveal');
  if (reduce || !('IntersectionObserver' in window)) {
    items.forEach((el) => el.classList.add('in'));
  } else {
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            e.target.classList.add('in');
            io.unobserve(e.target);
          }
        }
      },
      { rootMargin: '0px 0px -8% 0px' },
    );
    items.forEach((el) => io.observe(el));
  }

  const frame = document.querySelector('.hero-shot .frame');
  if (frame && !reduce) {
    let queued = false;
    const tilt = () => {
      queued = false;
      const t = Math.min(1, Math.max(0, window.scrollY / 420));
      frame.style.setProperty('--tilt', `${(8 * (1 - t)).toFixed(2)}deg`);
    };
    addEventListener('scroll', () => {
      if (!queued) {
        queued = true;
        requestAnimationFrame(tilt);
      }
    }, { passive: true });
    tilt();
  }

  const tabs = [...document.querySelectorAll('.tour-tabs [role="tab"]')];
  const caption = document.querySelector('.tour-caption');
  const select = (tab, focus) => {
    for (const t of tabs) {
      const on = t === tab;
      t.setAttribute('aria-selected', String(on));
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute('aria-controls'))?.classList.toggle('on', on);
    }
    if (caption) caption.textContent = tab.dataset.caption ?? '';
    if (focus) tab.focus();
  };
  tabs.forEach((tab, i) => {
    tab.addEventListener('click', () => select(tab, false));
    tab.addEventListener('keydown', (e) => {
      const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
      if (d) {
        e.preventDefault();
        select(tabs[(i + d + tabs.length) % tabs.length], true);
      }
    });
  });
})();
