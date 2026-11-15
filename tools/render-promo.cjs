// Рендер рекламного ролика: headless Chrome проигрывает страницу promo/ в виртуальном
// времени, каждый кадр снимается и сразу уходит в ffmpeg вместе с музыкой.
//
//   node tools/render-promo.cjs [--seconds 46.8] [--fps 30] [--out output/flyx-promo.mp4]
//                               [--url /promo/] [--size 1920x1080] [--no-audio]
//
// Нужны: Chrome, ffmpeg (FFMPEG=путь или D:/rust/ffmpeg/bin/ffmpeg.exe).
const fs = require('fs');
const http = require('http');
const path = require('path');
const { spawn } = require('child_process');

const root = path.join(__dirname, '..');
const args = Object.fromEntries(process.argv.slice(2).join(' ').split('--').filter(Boolean).map((part) => {
  const [key, ...rest] = part.trim().split(/\s+/);
  return [key, rest.join(' ') || true];
}));
const fps = Number(args.fps) || 30;
const seconds = Number(args.seconds) || 46.8;
const [width, height] = String(args.size || '1920x1080').split('x').map(Number);
const out = path.resolve(root, args.out || 'output/flyx-promo.mp4');
const pageUrl = args.url || '/promo/';
const audio = path.join(root, 'ELLA_Superfresh_2_0.mp3');
const audioStart = Number(args['audio-start'] ?? 8.44);   // секунда трека, с которой начинается ролик
const ffmpeg = process.env.FFMPEG || 'D:/rust/ffmpeg/bin/ffmpeg.exe';
const chromePath = process.env.CHROME || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const PORT = 5180, DEBUG_PORT = 9231;

const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.json': 'application/json',
  '.png': 'image/png', '.gif': 'image/gif', '.webp': 'image/webp', '.webm': 'video/webm', '.ttf': 'font/ttf', '.tgs': 'application/octet-stream', '.svg': 'image/svg+xml', '.mp3': 'audio/mpeg' };

// /promo/* — страница ролика, всё остальное — настоящий интерфейс из ui/
const server = http.createServer((request, response) => {
  let name = path.normalize(decodeURIComponent(request.url.split('?')[0])).replace(/^[\\/]+/, '');
  let base = path.join(root, 'ui');
  if (name === 'promo' || name.startsWith('promo' + path.sep)) { base = path.join(root, 'promo'); name = name.slice(6); }
  let file = path.join(base, name || 'index.html');
  if (base.endsWith('promo') && name === 'music.mp3') file = audio;
  if (file !== audio && !file.startsWith(base)) { response.writeHead(403).end(); return; }
  fs.readFile(file, (error, data) => {
    if (error) { response.writeHead(404).end('not found'); return; }
    response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(data);
  });
});

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

(async () => {
  await new Promise((resolve) => server.listen(PORT, '127.0.0.1', resolve));
  if (args.serve) { console.log(`http://localhost:${PORT}${pageUrl}`); return; }

  const profile = path.join(root, 'output', 'chrome-profile-promo');
  const chrome = spawn(chromePath, ['--headless=new', '--no-first-run', '--no-default-browser-check', '--hide-scrollbars', '--autoplay-policy=no-user-gesture-required',
    '--force-device-scale-factor=1', `--remote-debugging-port=${DEBUG_PORT}`, `--user-data-dir=${profile}`, 'about:blank'], { windowsHide: true, stdio: 'ignore' });
  let tab;
  for (let i = 0; i < 80 && !tab; i++) { try { tab = await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/new?about:blank`, { method: 'PUT' })).json(); } catch { await sleep(100); } }
  if (!tab) throw Error('Chrome не запустился');

  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise((resolve) => { ws.onopen = resolve; });
  let seq = 0;
  const pending = new Map(), waiters = new Map(), errors = [];
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id) { const p = pending.get(message.id); pending.delete(message.id); message.error ? p.reject(Error(message.error.message)) : p.resolve(message.result); return; }
    if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text);
    const waiter = waiters.get(message.method);
    if (waiter) { waiters.delete(message.method); waiter(message.params); }
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => { const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
  const once = (method) => new Promise((resolve) => waiters.set(method, resolve));
  const evaluate = async (expression) => {
    const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };

  let encoder;
  try {
    await send('Runtime.enable'); await send('Page.enable');
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false });
    await send('Page.navigate', { url: `http://127.0.0.1:${PORT}${pageUrl}${pageUrl.includes('?') ? '&' : '?'}render=1` });
    // страница сама говорит, когда шрифты, картинки и встроенные окна готовы
    for (let i = 0; i < 300; i++) { if (await evaluate('window.promoReady === true || (window.promoReady === undefined && document.readyState === "complete")')) break; await sleep(100); }
    await sleep(500);

    const frames = Math.round(seconds * fps);
    const input = ['-y', '-v', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'png', '-i', '-'];
    const useAudio = !args['no-audio'] && fs.existsSync(audio);
    if (useAudio) input.push('-ss', String(audioStart), '-t', String(seconds), '-i', audio);
    const fade = `afade=t=in:st=0:d=0.4,afade=t=out:st=${(seconds - 1.6).toFixed(2)}:d=1.6`;
    const output = ['-c:v', 'libx264', '-preset', 'slow', '-crf', '16', '-pix_fmt', 'yuv420p', '-r', String(fps), '-movflags', '+faststart'];
    if (useAudio) output.push('-af', fade, '-c:a', 'aac', '-b:a', '256k', '-shortest');
    fs.mkdirSync(path.dirname(out), { recursive: true });
    encoder = spawn(ffmpeg, [...input, ...output, out], { stdio: ['pipe', 'inherit', 'inherit'] });
    const encoded = new Promise((resolve, reject) => encoder.on('exit', (code) => (code === 0 ? resolve() : reject(Error(`ffmpeg завершился с кодом ${code}`)))));

    // Время страницы останавливается и дальше идёт только по нашей команде, ровно по кадру.
    await send('Emulation.setVirtualTimePolicy', { policy: 'pause' });
    await evaluate('window.promoStart && window.promoStart(), true');
    const started = Date.now();
    for (let frame = 0; frame < frames; frame++) {
      const expired = once('Emulation.virtualTimeBudgetExpired');
      await send('Emulation.setVirtualTimePolicy', { policy: 'advance', budget: 1000 / fps });
      await expired;
      const { data } = await send('Page.captureScreenshot', { format: 'png', optimizeForSpeed: true });
      if (!encoder.stdin.write(Buffer.from(data, 'base64'))) await new Promise((resolve) => encoder.stdin.once('drain', resolve));
      if (frame % (fps * 5) === 0) console.log(`кадр ${frame}/${frames}, ${((Date.now() - started) / 1000).toFixed(0)} с`);
    }
    encoder.stdin.end();
    await encoded;
    if (errors.length) console.log('Ошибки на странице:', [...new Set(errors)].slice(0, 5));
    console.log(`Готово: ${out} (${(fs.statSync(out).size / 1048576).toFixed(1)} МБ, ${frames} кадров)`);
  } finally {
    try { await send('Browser.close'); } catch { chrome.kill(); }
    ws.close();
    server.close();
    if (encoder && encoder.exitCode === null) encoder.stdin.destroy();
  }
})().catch((error) => { console.error(error); server.close(); process.exitCode = 1; });
