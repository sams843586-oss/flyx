const {spawn}=require('child_process'),path=require('path'),fs=require('fs');
const settingsTest=process.argv.includes('--settings');
const settingsData=path.resolve('output/native-settings-data');
if(settingsTest){fs.mkdirSync(path.join(settingsData,'FlexConnect'),{recursive:true});fs.writeFileSync(path.join(settingsData,'FlexConnect/state.json'),JSON.stringify({profiles:[],preferences:{theme:'dark'}}));}
async function run(args,port){
 const child=spawn('D:/rust/target/flexconnect/debug/Flyx.exe',args,{windowsHide:true,stdio:'ignore',env:{...process.env,...(settingsTest?{APPDATA:settingsData}:{}),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-port='+port,WEBVIEW2_USER_DATA_FOLDER:path.resolve('output/native-profile-'+port)}});
 let ws;
 try{
  let tab;for(let i=0;i<150;i++){try{const tabs=await(await fetch('http://127.0.0.1:'+port+'/json')).json();tab=tabs.find(t=>t.type==='page'&&!t.url.startsWith('about:'));if(tab)break;}catch{}await new Promise(r=>setTimeout(r,100));}
  if(!tab)throw Error('Native WebView did not become ready');
  ws=new WebSocket(tab.webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);
  let seq=0;const pending=new Map(),errors=[];ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);pending.delete(m.id);m.error?p.reject(m.error):p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params.exceptionDetails.text);};
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
  const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};
  await send('Runtime.enable');
  await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
  for(let i=0;i<100;i++){if(await evaluate('!!window.__TAURI__ && !!window.FlyxCountries && !!window.FlyxStickers'))break;await new Promise(r=>setTimeout(r,100));}
  await new Promise(r=>setTimeout(r,1200));
  if(settingsTest && port===9225){
    const stable=await evaluate(`(async()=>{
      const chips=[$('chip-mode'),$('chip-split')];
      const original=chips.map(n=>n.firstChild);
      let writes=0;
      const observer=new MutationObserver(records=>writes+=records.length);
      chips.forEach(n=>observer.observe(n,{subtree:true,childList:true,characterData:true,attributes:true}));
      await new Promise(r=>setTimeout(r,2300));
      observer.disconnect();
      return writes===0 && chips.every((n,i)=>n.firstChild===original[i]);
    })()`);
    if(!stable)throw Error('Native status polling rewrote mode chips');
    console.log('Native mode chips stable across status polling: OK');
    await evaluate("showPage('settings')");
    await new Promise(r=>setTimeout(r,400));
    await evaluate("$('settings-mascot').decode()");
    if(await evaluate("!$('settings-mascot').getAttribute('src').endsWith('.webp') || $('settings-mascot').naturalWidth!==256"))throw Error('Settings mascot did not animate: '+await evaluate("JSON.stringify({src:$('settings-mascot').getAttribute('src'),width:$('settings-mascot').naturalWidth,visible:visibleMascots.has($('settings-mascot')),hidden:document.hidden,background:window.FlyxBackground,reduced:settingsMotion.matches,pageHidden:$('page-settings').hidden})"));
    const {data}=await send('Page.captureScreenshot',{format:'png'});
    fs.writeFileSync('output/flyx-settings-mascot.png',Buffer.from(data,'base64'));
    await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'reduce'}]});
    await new Promise(r=>setTimeout(r,100));
    if(await evaluate("!$('settings-mascot').getAttribute('src').endsWith('-still.png')"))throw Error('Settings mascot ignored reduced motion');
    await send('Emulation.setEmulatedMedia',{features:[{name:'prefers-reduced-motion',value:'no-preference'}]});
    await evaluate("showPage('home')");
    if(await evaluate("!$('settings-mascot').getAttribute('src').endsWith('-still.png')"))throw Error('Hidden settings mascot kept animating');
    console.log('Native settings mascot: animated when visible, static when hidden/reduced motion: OK');
    for(const page of ['servers','rules']) {
      await evaluate("showPage('"+page+"')");
      await new Promise(r=>setTimeout(r,300));
      await evaluate("$('"+page+"-mascot').decode()");
      if(await evaluate("$('"+page+"-mascot').getAttribute('src')!=='assets/page-mascot.webp'"))throw Error('Native page GIF failed '+page);
    }
    await evaluate("showPage('home')");
    if(await evaluate("pageMascots.some(n=>!n.getAttribute('src').endsWith('-still.png'))"))throw Error('Hidden native page GIF kept animating');
    console.log('Native server/rules GIFs and tab hiding: OK');
    if(process.argv.includes('--release-check')) {
      const update=await evaluate('__TAURI__.core.invoke("check_update")');
      const expected=process.argv.find(arg=>arg.startsWith('--expected-update='))?.split('=')[1];
      if(expected ? update?.version!==expected : update!==null)throw Error('Unexpected update result: '+JSON.stringify(update));
      console.log('Live signed GitHub release endpoint: '+(expected?'new version '+expected+' is available':'current version correctly reports no update'));
    }

  }
  if(process.argv.includes('--live-import') && !args.length) {
    const statePath=path.join(process.env.APPDATA,'FlexConnect/state.json');
    const saved=JSON.parse(fs.readFileSync(statePath));
    const source=saved.profiles.find(p=>p.name.includes('elix'))?.source;
    if(!source)throw Error('No elix subscription found');
    const imported=await evaluate('(async()=>{const r=await __TAURI__.core.invoke("import_source",{input:'+JSON.stringify(source)+'});return {nodes:r.nodes.length,skipped:r.skipped.map(s=>s.scheme),notices:r.nodes.filter(n=>/приложение не поддерживается|рекомендуемые приложения/i.test(n.name)).length}})()');
    console.log('Native elix import',imported);
    if(imported.nodes<1 || imported.notices)throw Error('Native live subscription import failed');
  }
  if(settingsTest) {
    const expected=port===9225?'dark':'light';
    if(await evaluate('document.documentElement.dataset.theme')!==expected)throw Error('Theme was not restored');
    if(port===9225){
      await evaluate("document.querySelector('[data-theme-choice=light]').click()");
      await new Promise(r=>setTimeout(r,700));
      await evaluate("$('language-select').value='en';$('language-select').dispatchEvent(new Event('change'));$('start-hidden').click()");
      await new Promise(r=>setTimeout(r,700));
      await evaluate('__TAURI__.core.invoke("set_ui_language",{language:"fa"})');
      await evaluate('__TAURI__.core.invoke("set_ui_language",{language:"en"})');
      const saved=JSON.parse(fs.readFileSync(path.join(settingsData,'FlexConnect/state.json')));
      if(saved.preferences.language!=='en'||!saved.preferences.startHidden)throw Error('Native language/start-hidden preferences not persisted');
      if(saved.preferences.theme!=='light')throw Error('Theme was not persisted');
    }
    if(port!==9225 && await evaluate('document.documentElement.lang')!=='en')throw Error('Native language not restored');
    console.log('Native theme/language/start-hidden persistence',expected,'OK');
  }
  const visible=await evaluate('__TAURI__.window.getCurrentWindow().isVisible()');
  console.log('Native launch',args.length?'background':'normal',{visible,background:await evaluate('!!window.FlyxBackground'),errors:await evaluate('FlyxStickers.errors.concat(FlyxCountries.errors)')});
  let storedHidden=false;try{storedHidden=JSON.parse(fs.readFileSync(path.join(settingsTest?settingsData:process.env.APPDATA,'FlexConnect/state.json'))).preferences?.startHidden===true}catch{}
  const expectedVisible=port===9225&&settingsTest?true:!args.includes('--background')&&!storedHidden;
  if(visible!==expectedVisible)throw Error('Initial native visibility mismatch');
  if(visible){await evaluate("document.getElementById('win-close').click()");await new Promise(r=>setTimeout(r,400));const after=await evaluate('__TAURI__.window.getCurrentWindow().isVisible()');
   const hidden=await evaluate('document.hidden');console.log('Native close-to-tray',{visible:after,processAlive:child.exitCode===null,documentHidden:hidden,flagsPaused:await evaluate("[...document.querySelectorAll('.country-flag video')].every(v=>v.paused)")});
   if(await evaluate("!window.FlyxBackground || [...document.querySelectorAll('.country-flag video')].some(v=>!v.paused) || lottie.getRegisteredAnimations().some(a=>!a.isPaused) || !!document.getElementById('mascot').getAttribute('src')"))throw Error('Hidden window animations did not stop');
   if(after||child.exitCode!==null)throw Error('Native close did not keep process in background');
  }
  if(errors.length)throw Error(errors.join('; '));
 }finally{ws?.close();child.kill();}
}
(async()=>{await run([],9225);await run(['--background'],9226);if(settingsTest)await run([],9227)})().catch(e=>{console.error(e);process.exit(1)});
