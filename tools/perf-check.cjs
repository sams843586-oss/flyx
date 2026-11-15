// Замер нагрузки интерфейса: сколько процессорного времени все процессы Chrome
// тратят на 10 секунд простоя каждой страницы, и сколько весит загрузка.
//   node tools/preview-server.js 5173   (в другом окне)
//   node tools/perf-check.cjs
const { spawn } = require('child_process');
const path = require('path');

const SECONDS = Number(process.env.PERF_SECONDS) || 10;
// PERF_CSS — добавочные стили, чтобы проверить вклад отдельной анимации; PERF_ONLY — номер сцены
const EXTRA_CSS = process.env.PERF_CSS || '';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

(async () => {
  await fetch('http://127.0.0.1:5173').then((r) => { if (!r.ok) throw Error('Сервер предпросмотра не запущен'); });
  const chrome = spawn('C:/Program Files/Google/Chrome/Application/chrome.exe', ['--headless=new', '--no-first-run', '--no-default-browser-check',
    '--remote-debugging-port=9233', '--user-data-dir=' + path.resolve('output/chrome-profile-perf'), 'about:blank'], { windowsHide: true, stdio: 'ignore' });
  let version;
  for (let i = 0; i < 80 && !version; i++) { try { version = await (await fetch('http://127.0.0.1:9233/json/version')).json(); } catch { await sleep(100); } }
  if (!version) throw Error('Chrome не запустился');
  const tab = await (await fetch('http://127.0.0.1:9233/json/new?about:blank', { method: 'PUT' })).json();

  const connect = async (url) => {
    const ws = new WebSocket(url);
    await new Promise((resolve) => { ws.onopen = resolve; });
    let seq = 0; const pending = new Map();
    ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id) { const p = pending.get(m.id); pending.delete(m.id); m.error ? p.reject(Error(m.error.message)) : p.resolve(m.result); } };
    return { ws, send: (method, params = {}) => new Promise((resolve, reject) => { const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); }) };
  };
  const browser = await connect(version.webSocketDebuggerUrl);
  const page = await connect(tab.webSocketDebuggerUrl);
  const evaluate = async (expression) => (await page.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })).result.value;
  const cpu = async () => (await browser.send('SystemInfo.getProcessInfo')).processInfo.reduce((sum, p) => sum + p.cpuTime, 0);

  try {
    await page.send('Page.enable'); await page.send('Network.enable');
    await page.send('Emulation.setDeviceMetricsOverride', { width: 420, height: 760, deviceScaleFactor: 1.5, mobile: false });
    let bytes = 0;
    page.ws.addEventListener('message', (e) => { const m = JSON.parse(e.data); if (m.method === 'Network.loadingFinished') bytes += m.params.encodedDataLength; });
    await page.send('Page.navigate', { url: 'http://127.0.0.1:5173' });
    await sleep(2500);
    console.log(`загрузка главной: ${(bytes / 1048576).toFixed(2)} МБ`);

    const scenes = [
      ['главная, отключено', ''],
      ['главная, подключено', `mockInvoke.state.tunnel='connected'; mockInvoke.state.since=Math.floor(Date.now()/1000); live.service='ok'; live.tunnel='connected'; live.since=mockInvoke.state.since; renderHome();`],
      ['серверы', `mergeImported({source:'https://x.example/s',title:'Demo',skipped:[],nodes:['Нидерланды','Германия','Финляндия','Швеция','Япония'].map((name,i)=>({name,server:'s'+i+'.example',port:443,proto:'vless',transport:'tcp',security:'reality',via_xray:false,node:{name}}))}); showPage('servers');`],
      ['правила', `showPage('rules');`],
      ['настройки', `showPage('settings');`],
    ];
    if (EXTRA_CSS) await evaluate('document.head.insertAdjacentHTML("beforeend", "<style>" + ' + JSON.stringify(EXTRA_CSS) + ' + "</style>")');
    const chosen = process.env.PERF_ONLY ? [scenes[Number(process.env.PERF_ONLY)]] : scenes;
    for (const [name, setup] of chosen) {
      if (setup) await evaluate(setup);
      if (process.env.PERF_JS) await evaluate(process.env.PERF_JS);
      await sleep(1500);
      const before = await cpu();
      await sleep(SECONDS * 1000);
      const used = (await cpu()) - before;
      console.log(`${name.padEnd(22)} ${(used / SECONDS * 100).toFixed(1).padStart(5)} % одного ядра`);
    }
  } finally {
    try { await browser.send('Browser.close'); } catch { chrome.kill(); }
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
