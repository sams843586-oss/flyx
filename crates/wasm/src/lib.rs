//! Shared subscription parser and core config generator for the Android WebView.
use wasm_bindgen::prelude::*;
use serde_json::{json,Value};
use flexconnect_core::{engine,ipc::{ConnectRequest,TunnelMode,SplitMode},subscription::parse_subscription};

#[wasm_bindgen]
pub fn parse_source(body: &str) -> Result<String, JsValue> {
    let parsed=parse_subscription(body);
    let nodes: Vec<Value>=parsed.nodes.into_iter().map(|node| json!({
        "name":node.name,"server":node.server,"port":node.port,
        "proto":node.proto_name(),"transport":node.transport_name(),"security":node.security_name(),
        "via_xray":engine::uses_xray(&node),"node":node
    })).collect();
    if nodes.is_empty(){return Err(JsValue::from_str("серверов не найдено: проверьте формат подписки"));}
    let skipped:Vec<Value>=parsed.skipped.into_iter().map(|s|json!({"line":s.line,"scheme":s.scheme,"reason":s.reason})).collect();
    Ok(json!({"nodes":nodes,"skipped":skipped}).to_string())
}

#[wasm_bindgen]
pub fn android_plan(request:&str,proxy_port:u16,xray_port:u16)->Result<String,JsValue>{
    let mut req:ConnectRequest=serde_json::from_str(request).map_err(|e|JsValue::from_str(&e.to_string()))?;
    // Android per-app routing is configured through VpnService package lists.
    // The native cores only need a loopback SOCKS listener and domain rules.
    req.mode=TunnelMode::Proxy;req.proxy_port=proxy_port;req.split=SplitMode::All;req.apps.clear();
    let opts=req.options().map_err(|e|JsValue::from_str(&e))?;
    let mut plan=engine::plan(&req.node,&opts,xray_port).map_err(|e|JsValue::from_str(&e.to_string()))?;
    for section in ["route","dns"] {
        if let Some(rules)=plan.singbox[section]["rules"].as_array_mut(){rules.retain(|rule|rule.get("process_name").is_none());}
    }
    Ok(json!({"singbox":plan.singbox,"xray":plan.xray,"name":req.node.name,"proxyPort":proxy_port}).to_string())
}
