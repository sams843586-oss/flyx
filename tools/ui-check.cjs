const fs = require('fs');
const { spawn } = require('child_process');
const path = require('path');
(async () => {
  await fetch('http://127.0.0.1:5173').then(r=>{if(!r.ok)throw Error('Preview server not ready')});
  const chrome = spawn('C:/Program Files/Google/Chrome/Application/chrome.exe', ['--headless=new', '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=9224', '--user-data-dir=' + path.resolve('output/chrome-profile'), 'about:blank'], {windowsHide: true, stdio: 'ignore'});
  let tab;
  for(let i=0;i<50;i++){try{tab=await (await fetch('http://127.0.0.1:9224/json/new?http://127.0.0.1:5173',{method:'PUT'})).json();break;}catch{await new Promise(r=>setTimeout(r,100));}}
  if(!tab) throw Error('Chrome did not start');
  const ws = new WebSocket(tab.webSocketDebuggerUrl);
  await new Promise(r=>ws.onopen=r);
  let seq=0; const pending=new Map(),errors=[];
  ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails.text);};
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
  const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};
  await send('Runtime.enable'); await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride',{width:420,height:760,deviceScaleFactor:1,mobile:false});
  await send('Page.navigate',{url:'http://127.0.0.1:5173'});
  await new Promise(r=>setTimeout(r,1200));
  await evaluate(`document.fonts.ready.then(()=>true)`);
  await send('DOM.enable'); await send('CSS.enable');
  const dom=await send('DOM.getDocument');
  const titleNode=await send('DOM.querySelector',{nodeId:dom.root.nodeId,selector:'#status-title'});
  const fonts=await send('CSS.getPlatformFontsForNode',{nodeId:titleNode.nodeId});
  console.log('Actual title fonts',fonts.fonts);
  if(!fonts.fonts.some(f=>f.isCustomFont&&f.familyName.includes('Inter')))throw Error('Inter did not render');
  console.log('Layout',await evaluate(`JSON.stringify({title:document.title,adVisible:!$('ad-slot').hidden,galleryRemoved:!$('sticker-gallery'),images:[...document.images].map(i=>({src:i.getAttribute('src'),loaded:i.complete&&i.naturalWidth>0})),adBottom:$('ad-slot').getBoundingClientRect().bottom,navTop:document.querySelector('.tabbar').getBoundingClientRect().top,scrollWidth:document.body.scrollWidth})`));
  async function shot(name){const {data}=await send('Page.captureScreenshot',{format:'png'});fs.writeFileSync('output/'+name+'.png',Buffer.from(data,'base64'));}
  await shot('flyx-home');
  const stableChips=await evaluate(`(async()=>{
    renderHome();
    const chips=[$('chip-mode'),$('chip-split')];
    const original=chips.map(n=>n.firstChild);
    let writes=0;
    const observer=new MutationObserver(records=>writes+=records.length);
    chips.forEach(n=>observer.observe(n,{subtree:true,childList:true,characterData:true,attributes:true}));
    for(let i=0;i<10;i++)renderHome();
    await new Promise(r=>setTimeout(r,2300));
    observer.disconnect();
    return writes===0 && chips.every((n,i)=>n.firstChild===original[i]);
  })()`);
  if(!stableChips)throw Error('Unchanged status polling rewrote mode chips');
  console.log('Mode chips remain stable across repeated renders and status polling: OK');

  await evaluate(`(async()=>{mockInvoke.state.tunnel='connected';live.service='ok';live.tunnel='connected';live.since=Math.floor(Date.now()/1000)-125;renderHome();await new Promise(r=>{const v=$('mascot');v.readyState>=2?r():v.addEventListener('loadeddata',r,{once:true})})})()`); await shot('flyx-connected');
  await evaluate(`showPage('rules')`);await new Promise(r=>setTimeout(r,500));await shot('flyx-rules');
  for(const [width,height] of [[400,720],[360,640],[520,860]]) {
    await send('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
    await evaluate("showPage('home')");
    console.log('Responsive',await evaluate(`JSON.stringify({width:innerWidth,height:innerHeight,contentWidth:document.body.scrollWidth,adBottom:$('ad-slot').getBoundingClientRect().bottom,navTop:document.querySelector('.tabbar').getBoundingClientRect().top,homeOverflow:$('page-home').scrollHeight>$('page-home').clientHeight,mascot:$('mascot').getAttribute('src')})`));
    const fit=await evaluate("document.body.scrollWidth===innerWidth && $('page-home').scrollHeight<=$('page-home').clientHeight && $('ad-slot').getBoundingClientRect().bottom<=document.querySelector('.tabbar').getBoundingClientRect().top");
    if(!fit) throw Error('Layout does not fit '+width+'x'+height);
    await shot('flyx-'+width+'x'+height);
  }
  await send('Emulation.setDeviceMetricsOverride',{width:420,height:760,deviceScaleFactor:1,mobile:false});
  await evaluate("showPage('home'); live.busy=true; renderHome()");
  await new Promise(r=>setTimeout(r,500));
  console.log('TGS loading',await evaluate("JSON.stringify({errors:FlyxStickers.errors,active:FlyxStickers.activeCount,power:$('power-sticker').dataset.sticker,svg:!!$('power-sticker').querySelector('svg')})"));
  if(await evaluate("FlyxStickers.errors.length>0 || !$('power-sticker').querySelector('svg')")) throw Error('TGS failed to render');
  await evaluate("live.busy=false; renderHome(); showPage('servers'); $('add-input').value='https://demo.example'; $('add-form').requestSubmit()");
  await new Promise(r=>setTimeout(r,800));
  await new Promise(r=>setTimeout(r,500));
  console.log('Country flags',await evaluate("JSON.stringify({countries:[...document.querySelectorAll('.node .country-flag')].map(n=>n.dataset.country),ready:[...document.querySelectorAll('.node video')].map(v=>({loaded:v.readyState>=2,playing:!v.paused})),errors:FlyxCountries.errors})"));
  if(await evaluate("FlyxCountries.errors.length || [...document.querySelectorAll('.node video')].some(v=>v.readyState<2)"))throw Error('Flag WebM failed');
  const countryCases=await evaluate("['🇳🇱 Amsterdam','ГЕРМАНИЯ 2','🇺🇸 США','Казахстан','Unknown node'].map(n=>FlyxCountries.country(n)).join(',')");
  if(countryCases!=='NL,DE,US,KZ,')throw Error('Country detection mismatch: '+countryCases);
  await shot('flyx-servers');
  if(await evaluate("FlyxStickers.errors.length>0 || !document.querySelector('.icon-button [data-sticker=remove] svg')"))throw Error('Profile TGS failed');
  await evaluate('(async()=>{await connect();showPage("rules")})()');
  if(await evaluate("!$('apply').hidden"))throw Error('Apply visible without edits');
  await evaluate("state.settings.ruDirect=true; renderHome()");
  if(await evaluate("$('apply').hidden"))throw Error('Apply did not appear after routing edit');
  await evaluate("state.settings.ruDirect=false; renderHome()");
  if(await evaluate("!$('apply').hidden"))throw Error('Apply remained visible after reverting');
  await evaluate("const n=selectedNode().node; const originalNodeName=n.name; n.name+=' renamed'; selectedNode().node=Object.fromEntries(Object.entries(n).reverse()); renderHome()");
  if(await evaluate("!$('apply').hidden"))throw Error('Display label or field order triggered Apply');
  await evaluate("selectedNode().node.name=originalNodeName; renderHome()");
  await evaluate('(async()=>{state.settings.ruDirect=true; const applying=connect();state.settings.ruDirect=false;await applying;})()');
  if(await evaluate("$('apply').hidden"))throw Error('Edits during apply were incorrectly marked applied');
  await evaluate('(async()=>{await connect()})()');
  if(await evaluate("!$('apply').hidden"))throw Error('Apply remained visible after success');
  console.log('Apply button: unchanged/reverted/applied hidden; edited visible; in-flight edits preserved');
  await evaluate("state.settings.ruDirect=true; renderRules(); renderHome()");
  await new Promise(r=>setTimeout(r,300)); await shot('flyx-apply');
  await evaluate("state.settings.ruDirect=false; renderHome()");
  await evaluate("showPage('home')");
  await shot('flyx-connected');
  await evaluate("mockInvoke.state.tunnel='idle'; live.tunnel='idle'; renderHome()");
  await new Promise(r=>setTimeout(r,300)); await shot('flyx-home');
  await evaluate("showPage('settings')");
  const themeGradients=[];
  for(const theme of ['light','dark','flyx']) {
    await evaluate("document.querySelector('[data-theme-choice="+theme+"]').click()");
    await new Promise(r=>setTimeout(r,100));
    if(await evaluate("document.documentElement.dataset.theme")!==theme)throw Error('Theme did not apply');
    if(await evaluate("!$('apply').hidden"))throw Error('Theme incorrectly requires reconnect');
    themeGradients.push(await evaluate("getComputedStyle($('power')).backgroundImage"));
    if(await evaluate("getComputedStyle($('apply')).backgroundImage!==getComputedStyle($('power')).backgroundImage"))throw Error('Button gradients disagree with theme');
    await shot('flyx-settings-'+theme);
    await evaluate("showPage('home')");await new Promise(r=>setTimeout(r,400));await shot('flyx-home-'+theme);
    await evaluate("showPage('settings')");
  }
  if(new Set(themeGradients).size!==3)throw Error('Theme button gradients did not change');
  await evaluate("showPage('servers')");
  await evaluate('(async()=>{await connect();await selectNode(state.profiles[0].id,1)})()');
  if(await evaluate("!$('apply').hidden || live.server !== selectedNode().node.name"))throw Error('Server change did not reconnect automatically');
  console.log('Themes and automatic server switching: OK');
  for(const page of ['servers','rules','settings']) {
    await evaluate("showPage('"+page+"');$('page-"+page+"').scrollTop=0");
    await new Promise(r=>setTimeout(r,300));
    await evaluate("$('"+page+"-mascot').decode()");
    if(await evaluate("$('"+page+"-mascot').getAttribute('src')!=='assets/"+(page==='settings'?'settings-mascot.webp':'page-mascot.webp')+"'"))throw Error('Missing animated mascot '+page);
    await shot('flyx-'+page+'-mascot');
  }
  await evaluate("window.FlyxBackground=true;window.dispatchEvent(new Event('flyx-background'))");
  if(await evaluate("pageMascots.some(image=>!image.getAttribute('src').endsWith('-still.png'))"))throw Error('Background page mascots kept animating');
  await evaluate("window.FlyxBackground=false;window.dispatchEvent(new Event('flyx-background'));showPage('home')");
  if(await evaluate("pageMascots.some(image=>!image.getAttribute('src').endsWith('-still.png'))"))throw Error('Hidden page mascots kept animating');
  console.log('Settings, servers and rules mascots: visible/background switching OK');

  await evaluate("showPage('settings')");
  for(const code of ['en','ru','zh','fa','ko','tk','uz','az','kk']) {
    await evaluate("$('language-select').value='"+code+"';$('language-select').dispatchEvent(new Event('change'))");
    await new Promise(r=>setTimeout(r,160));
    const valid=await evaluate("document.documentElement.lang==='"+code+"' && document.querySelector('#page-settings h2').textContent===t('settings') && $('add-input').placeholder===t('sourcePlaceholder') && [...document.querySelectorAll('[data-i18n]')].every(n=>Object.hasOwn(FlyxI18n.dictionaries['"+code+"'],n.dataset.i18n))");
    if(!valid)throw Error('Incomplete UI translation '+code);
    if(await evaluate("document.body.scrollWidth>innerWidth || $('page-settings').scrollWidth>$('page-settings').clientWidth"))throw Error('Language overflow '+code);
    if(await evaluate("document.documentElement.dir")!==(code==='fa'?'rtl':'ltr'))throw Error('Wrong language direction');
    await shot('flyx-language-'+code);
  }
  await evaluate("applyLanguage('ru');showPage('rules');state.settings.ruDirect=true;renderHome()");
  const catalog=await evaluate("({n:RU_DOMAINS.length,domains:connectRequest().direct_domains})");
  for(const domain of ['ru','xn--p1ai','ozon.ru','tbank.ru','lenta.com','okko.tv','yandex.net'])if(!catalog.domains.includes(domain))throw Error('Missing RU domain '+domain);
  if(catalog.domains.includes('com')||catalog.domains.includes('net'))throw Error('Overbroad direct rule');
  console.log('RU direct domains',catalog.n);
  await evaluate("state.settings.ruDirect=false;renderHome();showPage('settings');$('auto-connect').click();$('start-hidden').click()");
  await new Promise(r=>setTimeout(r,450));
  if(await evaluate("!mockInvoke.state.saved.preferences.autoConnect || !mockInvoke.state.saved.preferences.startHidden || !$('apply').hidden"))throw Error('Preference persistence or reconnect issue');
  await evaluate("$('auto-connect').click();$('start-hidden').click()");
  if(await evaluate("!document.querySelector('[data-sticker=settings] svg') || FlyxStickers.errors.length"))throw Error('Animated settings gear missing');
  console.log('Nine languages, startup preferences and animated settings: OK');
  await evaluate("window.__FLYX_PREVIEW_UPDATE__={version:'0.2.0',notes:'Release notes <script>not executable</script>'}");
  for(const code of ['en','ru','zh','fa','ko','tk','uz','az','kk']) {
    await evaluate("applyLanguage('"+code+"');FlyxUpdater.check(true)");
    await new Promise(r=>setTimeout(r,80));
    if(await evaluate("$('update-sheet').hidden||$('update-title').textContent!==t('updateAvailable')||$('update-install').textContent!==t('updateNow')||!!$('update-notes').querySelector('script')"))throw Error('Updater translation or unsafe release notes '+code);
  }
  await evaluate("applyLanguage('ru');FlyxUpdater.check(true)");
  await new Promise(r=>setTimeout(r,80));await shot('flyx-update-dialog');
  await evaluate("$('update-later').click();window.__FLYX_PREVIEW_UPDATE__=null;FlyxUpdater.check(true)");
  await new Promise(r=>setTimeout(r,80));
  if(await evaluate("$('update-title').textContent!==t('upToDate')||!$('update-install').hidden"))throw Error('No-update state incorrect');
  await evaluate("$('update-later').click()");
  if(fs.existsSync('ui/wasm/flyx_wasm.js')){
    const valid=await evaluate(`(async()=>{const wasm=await import('./wasm/flyx_wasm.js');await wasm.default();const parsed=JSON.parse(wasm.parse_source('vless://test-uuid@example.com:443?security=tls#Wasm'));const request={node:parsed.nodes[0].node,mode:'tun',split:'only',apps:['org.example.browser']};const plan=JSON.parse(wasm.android_plan(JSON.stringify(request),2080,20808));return parsed.nodes.length===1&&plan.xray!==null&&plan.singbox.inbounds[0].listen_port===2080&&plan.singbox.route.auto_detect_interface===false&&!plan.singbox.route.rules.some(r=>r.process_name);})()`);
    if(!valid)throw Error('Android shared Rust/Wasm config incorrect');
  }
  console.log('Updater nine languages/safe notes/current version and Android Wasm: OK');

  await evaluate("showPage('rules')");
  await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
  if(await evaluate("FlyxStickers.errors.length>0"))throw Error('TGS errors');
  await new Promise(r=>setTimeout(r,150));
  if(await evaluate("[...document.querySelectorAll('.country-flag video')].some(v=>!v.paused)"))throw Error('Reduced motion did not pause flags');
  console.log('Errors',errors);
  if(errors.length) throw Error(errors.join('; '));
  for(const [auto,alreadyConnected] of [[false,false],[true,false],[true,true]]) {
    const fixture={profiles:[{id:'auto',name:'Test',source:null,nodes:[{name:'Auto server',server:'example.com',port:443,proto:'vless',transport:'tcp',security:'tls',node:{name:'Auto server',server:'example.com',port:443}}]}],selected:{profile:'auto',index:0},preferences:{theme:'flyx',language:'en',autoConnect:auto,startHidden:false}};
    const init=await send('Page.addScriptToEvaluateOnNewDocument',{source:'window.__FLYX_PREVIEW_STATE__='+JSON.stringify(fixture)+';window.__FLYX_PREVIEW_TUNNEL__='+JSON.stringify(alreadyConnected?'connected':'idle')+';'});
    await send('Page.navigate',{url:'http://127.0.0.1:5173'});
    await new Promise(r=>setTimeout(r,2400));
    const calls=await evaluate('mockInvoke.state.connectCalls||0');
    if(calls!==(auto&&!alreadyConnected?1:0))throw Error('Auto-connect startup condition failed');
    await send('Page.removeScriptToEvaluateOnNewDocument',{identifier:init.identifier});
  }
  console.log('Auto-connect startup cases: OK');
  const linuxInit=await send('Page.addScriptToEvaluateOnNewDocument',{source:"window.__FLYX_PREVIEW_PLATFORM__='Linux';"});
  await send('Page.navigate',{url:'http://127.0.0.1:5173'});
  await new Promise(r=>setTimeout(r,1300));
  await evaluate("showPage('rules');state.settings.split='except';renderRules();$('app-input').value='MyBrowser';$('app-form').requestSubmit()");
  if(await evaluate("platformOS!=='Linux'||$('app-input').placeholder!=='firefox'||!state.settings.apps.includes('MyBrowser')||!t('about').includes('Linux')"))throw Error('Linux process names or platform label incorrect');
  await send('Page.removeScriptToEvaluateOnNewDocument',{identifier:linuxInit.identifier});
  console.log('Linux UI: preserves process case, no .exe suffix, correct platform label');

  await send('Browser.close');ws.close();
})().catch(e=>{console.error(e);process.exit(1);});
