'use strict';

// TGS = gzip-compressed Lottie JSON. Everything is bundled and works offline.
window.FlyxStickers = (() => {
  const names = new Set(['connect', 'disconnect', 'apply', 'add', 'remove', 'loading', 'server', 'home', 'rules', 'settings', 'ping']);
  const data = new Map();
  const mounted = new Map();
  const errors = [];
  const motion = matchMedia('(prefers-reduced-motion: reduce)');

  function load(name) {
    if (!data.has(name)) data.set(name, (async () => {
      const response = await fetch(`assets/stickers/${name}.tgs`);
      if (!response.ok) throw new Error(`TGS ${name}: ${response.status}`);
      const stream = response.body.pipeThrough(new DecompressionStream('gzip'));
      return new Response(stream).json();
    })());
    return data.get(name);
  }

  function play(record, restart = false) {
    const animation = record.animation;
    if (!animation || !animation.isLoaded) return;
    const visible = record.visible && !document.hidden && !window.FlyxBackground && !window.FlyxUnfocused;
    if (!visible) { animation.pause(); return; }
    if (motion.matches) {
      animation.goToAndStop(Math.floor(animation.totalFrames * .35), true);
    } else if (restart) {
      animation.goToAndPlay(0, true);
    } else if (record.name === 'loading') {
      animation.play();
    }
  }

  function unmount(node) {
    const record = mounted.get(node);
    if (!record) return;
    record.animation?.destroy();
    observer.unobserve(node);
    for (const type of ['pointerenter', 'focusin', 'click']) record.button?.removeEventListener(type, record.replay);
    mounted.delete(node);
  }

  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const record = mounted.get(entry.target);
      if (!record) continue;
      const wasVisible = record.visible;
      record.visible = entry.isIntersecting;
      play(record, record.visible && !wasVisible);
    }
  });

  async function mount(node) {
    const name = node.dataset.sticker;
    if (mounted.get(node)?.name === name) return;
    unmount(node);
    if (!names.has(name)) return;
    const record = { name, visible: false, animation: null, button: node.closest('button') };
    record.replay = () => play(record, true);
    mounted.set(node, record);
    observer.observe(node);
    for (const type of ['pointerenter', 'focusin', 'click']) record.button?.addEventListener(type, record.replay);
    try {
      const animationData = await load(name);
      if (mounted.get(node) !== record || !node.isConnected) return;
      record.animation = lottie.loadAnimation({
        container: node, renderer: 'svg', animationData: structuredClone(animationData),
        loop: name === 'loading', autoplay: false,
        rendererSettings: { progressiveLoad: false, preserveAspectRatio: 'xMidYMid meet' },
      });
      record.animation.addEventListener('DOMLoaded', () => play(record, true));
      record.animation.addEventListener('data_failed', () => errors.push(`Cannot render ${name}`));
    } catch (error) {
      errors.push(String(error));
      node.classList.add('sticker-unavailable');
    }
  }

  function scan(root) {
    if (root.matches?.('[data-sticker]')) mount(root);
    root.querySelectorAll?.('[data-sticker]').forEach(mount);
  }
  new MutationObserver(changes => {
    for (const change of changes) {
      if (change.type === 'attributes') mount(change.target);
      else change.addedNodes.forEach(scan);
    }
    for (const node of mounted.keys()) if (!node.isConnected) unmount(node);
  }).observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-sticker'] });
  document.addEventListener('visibilitychange', () => { for (const record of mounted.values()) play(record); });
  window.addEventListener('flyx-background', () => { for (const record of mounted.values()) play(record); });
  motion.addEventListener('change', () => { for (const record of mounted.values()) play(record, true); });
  scan(document.body);
  return {
    set(node, name) { if (node && node.dataset.sticker !== name) node.dataset.sticker = name; },
    errors,
    get activeCount() { return mounted.size; },
  };
})();
