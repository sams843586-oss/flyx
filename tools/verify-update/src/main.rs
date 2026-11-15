use std::{env,fs};
use base64::{Engine,engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey,Signature};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args:Vec<_>=env::args().collect();
    if args.len()!=4 {return Err("Usage: verify-update TAURI_CONFIG ARTIFACT SIGNATURE".into());}
    let config:serde_json::Value=serde_json::from_slice(&fs::read(&args[1])?)?;
    let key=String::from_utf8(STANDARD.decode(config["plugins"]["updater"]["pubkey"].as_str().ok_or("No updater public key")?)?)?;
    let signature=String::from_utf8(STANDARD.decode(fs::read_to_string(&args[3])?.trim())?)?;
    let (key,signature)=(PublicKey::decode(&key)?,Signature::decode(&signature)?);
    let mut bytes=fs::read(&args[2])?;
    key.verify(&bytes,&signature,true)?;
    if bytes.is_empty(){return Err("Empty installer".into());}
    bytes[0]^=1;
    assert!(key.verify(&bytes,&signature,true).is_err(),"Modified artifact accepted");
    println!("Signature verified; modified artifact rejected: {}",args[2]);
    Ok(())
}
