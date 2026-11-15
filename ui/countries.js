'use strict';

window.FlyxCountries = (() => {
  const files = {
    AM:'001', AU:'002', AT:'003', BD:'004', BY:'005', BE:'006', BW:'007', BR:'008', GB:'009', BG:'010',
    CA:'011', TD:'012', CN:'013', CI:'014', DK:'015', EE:'016', FI:'017', FR:'018', GF:'019', GA:'020',
    DE:'021', GH:'022', GR:'023', GN:'024', HU:'025', IS:'026', IN:'027', ID:'028', IE:'029', IL:'030',
    IT:'031', JP:'032', LV:'033', LT:'034', LU:'035', MU:'036', NL:'037', NO:'038', PK:'039', NG:'040',
    PE:'041', RO:'042', RU:'043', SL:'044', SK:'045', SI:'046', KR:'047', SR:'048', SE:'049', CH:'050',
    TH:'051', UA:'052', UY:'053', VN:'054', US:'055', SG:'056', CO:'057', BS:'058', PS:'059',
    'GB-ENG':'060', GL:'061', PL:'062',
  };
  const extras = {
    NL:['Голландия','Амстердам','Amsterdam','Holland','Rotterdam'],
    DE:['Франкфурт','Frankfurt','Берлин','Berlin'], FR:['Париж','Paris'],
    US:['США','USA','United States','Los Angeles','New York','Нью-Йорк'],
    GB:['Великобритания','UK','United Kingdom','Лондон','London'],
    'GB-ENG':['Англия','England'], RU:['Россия','России','Москва','Moscow'],
    FI:['Хельсинки','Helsinki'], SE:['Стокгольм','Stockholm'],
    CH:['Швейцария','Цюрих','Zurich'], KR:['Корея','South Korea','Сеул','Seoul'],
    BY:['Беларусь','Белоруссия'], CZ:['Чехия','Czechia'], KZ:['Казахстан','Kazakhstan'],
    TR:['Турция','Türkiye','Turkey'], ES:['Испания','Spain'], MK:['Македония','Macedonia'],
    EU:['Европа','Europe'], AE:['ОАЭ','UAE'], HK:['Гонконг','Hong Kong'],
  };
  const aliases = [];
  const regions = [...new Set([...Object.keys(files), ...Object.keys(extras)])];
  for (const code of regions) {
    const words = [...(extras[code] || []), code];
    if (code.length === 2) for (const locale of ['ru','en','zh','fa','ko','tk','uz','az','kk']) {
      const word = new Intl.DisplayNames([locale], { type:'region' }).of(code);
      if (word && word !== code) words.push(word);
    }
    for (const word of words) {
      const escaped = word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      aliases.push({ code, word, regex: new RegExp(`(?:^|[^\\p{L}])${escaped}(?=$|[^\\p{L}])`, 'iu') });
    }
  }
  aliases.sort((a,b) => b.word.length-a.word.length);
  const regional = /[\u{1F1E6}-\u{1F1FF}]{2}/u;
  const england = /\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}/u;
  function country(name = '') {
    if (england.test(name)) return 'GB-ENG';
    const flag = String(name).match(regional)?.[0];
    if (flag) return [...flag].map(c => String.fromCharCode(c.codePointAt(0)-0x1f1e6+65)).join('');
    return aliases.find(alias => alias.regex.test(name))?.code || '';
  }
  function label(name = '') {
    return String(name).replace(england,'').replace(/[\u{1F1E6}-\u{1F1FF}]{2}\uFE0F?/gu,'').trim();
  }

  const videos = new Map();
  const errors = [];
  const motion = matchMedia('(prefers-reduced-motion: reduce)');
  function update(record) {
    if (record.visible && !document.hidden && !window.FlyxBackground && !window.FlyxUnfocused && !motion.matches) record.video.play().catch(() => {});
    else record.video.pause();
  }
  const observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const record = videos.get(entry.target);
      if (!record) continue;
      record.visible = entry.isIntersecting;
      if (record.visible && !record.video.getAttribute('src')) record.video.src = record.src;
      update(record);
    }
  });
  function element(name) {
    const code = country(name);
    const root = document.createElement('span');
    root.className = 'country-flag';
    root.dataset.country = code;
    root.setAttribute('aria-hidden','true');
    const fallback = document.createElement('span');
    fallback.className = 'button-sticker flag-fallback';
    fallback.dataset.sticker = 'server';
    root.append(fallback);
    if (files[code]) {
      const video = document.createElement('video');
      video.muted = true; video.defaultMuted = true; video.loop = true; video.playsInline = true; video.preload = 'none';
      video.disablePictureInPicture = true;
      const record = { video, visible: false, src: `assets/flags/${code}.webm` };
      video.addEventListener('loadeddata', () => {
        root.dataset.ready = 'true'; fallback.remove(); update(record);
      });
      video.addEventListener('error', () => { errors.push(`WebM flag failed: ${code}`); });
      root.append(video);
      videos.set(root, record); observer.observe(root);
    }
    return root;
  }
  function set(root, name) {
    const code = country(name);
    if (root.dataset.country === code && root.firstChild) return;
    root.dataset.country = code;
    root.replaceChildren(element(name));
  }
  new MutationObserver(() => {
    for (const [root, record] of videos) if (!root.isConnected) {
      observer.unobserve(root); record.video.pause();
      record.video.removeAttribute('src'); record.video.load(); videos.delete(root);
    }
  }).observe(document.body,{ childList:true,subtree:true });
  document.addEventListener('visibilitychange', () => { for (const record of videos.values()) update(record); });
  window.addEventListener('flyx-background', () => { for (const record of videos.values()) update(record); });
  motion.addEventListener('change', () => { for (const record of videos.values()) update(record); });
  return { element, set, country, label, errors, files };
})();
