'use strict';
if (window.FlyxAndroid) {
  const pending = new Map();
  let sequence = 0;
  function call(command,args={}) {
    return new Promise((resolve,reject)=>{
      const id=String(++sequence);
      pending.set(id,{resolve,reject});
      window.FlyxAndroid.postMessage(JSON.stringify({id,command,args}));
    });
  }
  window.FlyxAndroidRPC={resolve(id,result,error){const item=pending.get(id);if(!item)return;pending.delete(id);error?item.reject(Error(error)):item.resolve(result);}};
  const core = import('./wasm/flyx_wasm.js').then(async module=>{await module.default();return module;});
  window.FlyxAndroidInvoke=async(command,args={})=>{
    if(command==='import_source') {
      const engine=await core;
      if(!/^https?:\/\//i.test(args.input.trim()))return {...JSON.parse(engine.parse_source(args.input)),source:null};
      let last;
      for(const agent of ['Flyx/0.1.0','v2rayNG/1.10.24','Happ/4.0','INCY/3.8.6']) {
        try {
          const result=await call('fetch_source',{url:args.input.trim(),agent});
          const parsed=JSON.parse(engine.parse_source(result.body));delete result.body;
          return {...result,...parsed,source:args.input.trim()};
        }catch(error){last=error;}
      }
      throw last;
    }
    if(command==='svc_connect') {
      const engine=await core,ports=await call('allocate_ports');
      const plan=JSON.parse(engine.android_plan(JSON.stringify(args.request),ports.proxy,ports.xray));
      return call(command,{plan,request:args.request});
    }
    return call(command,args);
  };
}
