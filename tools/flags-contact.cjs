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
  await send('Emulation.setDeviceMetricsOverride',{width:960,height:960,deviceScaleFactor:1,mobile:false});
  for(const start of [1,37]) {
    await send('Page.navigate',{url:'http://127.0.0.1:5174/flags.html?start='+start});
    for(let i=0;i<100;i++){if(await evaluate('window.ready===true'))break;await new Promise(r=>setTimeout(r,100));}
    const {data}=await send('Page.captureScreenshot',{format:'png'});fs.writeFileSync('output/flags-contact-'+start+'.png',Buffer.from(data,'base64'));
  }
  await send('Browser.close');ws.close();
})().catch(e=>{console.error(e);process.exitCode=1;});
