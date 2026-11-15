package app.flyx.android;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.net.VpnService;
import android.os.Bundle;
import android.util.AtomicFile;
import android.webkit.*;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;

public class MainActivity extends Activity {
    private WebView web;
    private final ExecutorService executor=Executors.newFixedThreadPool(3);
    private String pendingId;
    private JSONObject pendingArgs;
    private final File stateFile(){return new File(getFilesDir(),"state.json");}
    @Override public void onCreate(Bundle saved){
        super.onCreate(saved);
        getWindow().setStatusBarColor(0xff142343);getWindow().setNavigationBarColor(0xff142343);
        web=new WebView(this);setContentView(web);
        web.getSettings().setJavaScriptEnabled(true);web.getSettings().setDomStorageEnabled(true);
        web.getSettings().setAllowFileAccess(false);web.getSettings().setAllowContentAccess(false);
        web.getSettings().setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        web.addJavascriptInterface(new Bridge(),"FlyxAndroid");
        web.setWebViewClient(new WebViewClient(){
            @Override public WebResourceResponse shouldInterceptRequest(WebView view,WebResourceRequest request){
                Uri uri=request.getUrl();
                if(!"app.flyx.local".equals(uri.getHost()))return new WebResourceResponse("text/plain","UTF-8",new ByteArrayInputStream(new byte[0]));
                String path=uri.getPath();if(path==null||path.equals("/"))path="/index.html";
                if(path.contains(".."))return null;
                try{
                    String extension=path.substring(path.lastIndexOf('.')+1);
                    String mime=mimeType(extension);
                    return new WebResourceResponse(mime,"UTF-8",getAssets().open(path.substring(1)));
                }catch(Exception e){return new WebResourceResponse("text/plain","UTF-8",404,"Not Found",Collections.emptyMap(),new ByteArrayInputStream(new byte[0]));}
            }
            @Override public boolean shouldOverrideUrlLoading(WebView view,WebResourceRequest request){
                Uri uri=request.getUrl();if("app.flyx.local".equals(uri.getHost()))return false;
                if("https".equals(uri.getScheme()))startActivity(new Intent(Intent.ACTION_VIEW,uri));
                return true;
            }
        });
        web.loadUrl("https://app.flyx.local/index.html");
        if(android.os.Build.VERSION.SDK_INT>=33)requestPermissions(new String[]{"android.permission.POST_NOTIFICATIONS"},8);
    }
    private static String mimeType(String extension){
        switch(extension){
            case "html":return "text/html";case "js":return "text/javascript";case "css":return "text/css";
            case "json":return "application/json";case "wasm":return "application/wasm";case "webm":return "video/webm";
            case "woff2":return "font/woff2";case "gif":return "image/gif";case "png":return "image/png";case "webp":return "image/webp";
            default:return "application/octet-stream";
        }
    }
    private void reply(String id,Object result,String error){
        runOnUiThread(()->{
            if(isFinishing())return;
            String payload=result==null?"null":result instanceof String?JSONObject.quote((String)result):result.toString();
            web.evaluateJavascript("window.FlyxAndroidRPC&&window.FlyxAndroidRPC.resolve("+JSONObject.quote(id)+","+payload+","+(error==null?"null":JSONObject.quote(error))+")",null);
        });
    }
    class Bridge {
        @JavascriptInterface public void postMessage(String raw){
            if(raw.length()>9*1024*1024)return;
            executor.execute(()->{
                String id="";
                try{
                    JSONObject message=new JSONObject(raw);id=message.getString("id");String cmd=message.getString("command");JSONObject args=message.optJSONObject("args");if(args==null)args=new JSONObject();
                    switch(cmd){
                        case "platform_info":reply(id,"Android",null);break;
                        case "load_state":
                            if(!stateFile().exists()){reply(id,null,null);break;}
                            byte[] data=readLimited(new AtomicFile(stateFile()).openRead(),8*1024*1024);reply(id,new JSONObject(new String(data,StandardCharsets.UTF_8)),null);break;
                        case "save_state":{
                            byte[] bytes=args.getJSONObject("state").toString().getBytes(StandardCharsets.UTF_8);if(bytes.length>8*1024*1024)throw new IOException("Too much data");
                            AtomicFile file=new AtomicFile(stateFile());FileOutputStream out=null;
                            try{out=file.startWrite();out.write(bytes);file.finishWrite(out);}catch(Exception e){if(out!=null)file.failWrite(out);throw e;}
                            reply(id,null,null);break;
                        }
                        case "fetch_source":reply(id,fetch(args.getString("url"),args.optString("agent","Flyx/0.1.0")),null);break;
                        case "allocate_ports":{
                            try(ServerSocket one=new ServerSocket(0,0,InetAddress.getLoopbackAddress());ServerSocket two=new ServerSocket(0,0,InetAddress.getLoopbackAddress())){
                                reply(id,new JSONObject().put("proxy",one.getLocalPort()).put("xray",two.getLocalPort()),null);
                            }break;
                        }
                        case "svc_status":case "svc_logs":reply(id,FlyxVpnService.snapshot(),null);break;
                        case "svc_connect":{
                            String requestId=id;JSONObject config=args;
                            runOnUiThread(()->{
                                pendingId=requestId;pendingArgs=config;
                                Intent prepare=VpnService.prepare(MainActivity.this);
                                if(prepare!=null)startActivityForResult(prepare,7);else startPending();
                            });break;
                        }
                        case "svc_disconnect":{String requestId=id;FlyxVpnService.disconnect(MainActivity.this,result->reply(requestId,result,null));break;}
                        case "ping":{
                            long start=System.nanoTime();Integer latency=null;
                            try(java.net.Socket socket=new java.net.Socket()){socket.connect(new InetSocketAddress(args.getString("server"),args.getInt("port")),2000);latency=(int)((System.nanoTime()-start)/1000000);}catch(Exception ignored){}
                            reply(id,latency,null);break;
                        }
                        case "open_ad_url":{
                            Uri uri=Uri.parse(args.getString("url"));if(!"https".equals(uri.getScheme()))throw new IOException("HTTPS required");
                            runOnUiThread(()->startActivity(new Intent(Intent.ACTION_VIEW,uri)));reply(id,null,null);break;
                        }
                        case "set_ui_language":reply(id,null,null);break;
                        default:throw new IOException("Unknown Android command: "+cmd);
                    }
                }catch(Exception e){reply(id,null,e.getMessage()==null?e.toString():e.getMessage());}
            });
        }
    }
    private void startPending(){
        String id=pendingId;JSONObject args=pendingArgs;pendingId=null;pendingArgs=null;
        if(id!=null)FlyxVpnService.connect(this,args,result->reply(id,result,null));
    }
    @Override protected void onActivityResult(int request,int result,Intent data){super.onActivityResult(request,result,data);if(request==7){if(result==RESULT_OK)startPending();else{reply(pendingId,null,"Разрешение на VPN не выдано");pendingId=null;pendingArgs=null;}}}
    @Override protected void onPause(){super.onPause();web.evaluateJavascript("window.FlyxBackground=true;window.dispatchEvent(new Event('flyx-background'));if(typeof renderHome==='function')renderHome()",null);web.onPause();}
    @Override protected void onResume(){super.onResume();if(web!=null){web.onResume();web.evaluateJavascript("window.FlyxBackground=false;window.dispatchEvent(new Event('flyx-background'));if(typeof refreshStatus==='function')refreshStatus()",null);}}
    @Override public void onBackPressed(){moveTaskToBack(true);}
    private static byte[] readLimited(InputStream input,int max)throws IOException{try(InputStream in=input;ByteArrayOutputStream out=new ByteArrayOutputStream()){byte[] block=new byte[8192];int n;while((n=in.read(block))!=-1){if(out.size()+n>max)throw new IOException("Response too large");out.write(block,0,n);}return out.toByteArray();}}
    private JSONObject fetch(String source,String agent)throws Exception{
        URL url=new URL(source);
        String id=getPreferences(0).getString("subscriptionId",null);if(id==null){id=UUID.randomUUID().toString();getPreferences(0).edit().putString("subscriptionId",id).apply();}
        for(int redirect=0;redirect<6;redirect++){
            if(!url.getProtocol().equals("https"))throw new IOException("Android subscriptions require HTTPS");
            HttpURLConnection c=(HttpURLConnection)url.openConnection();c.setConnectTimeout(20000);c.setReadTimeout(20000);c.setInstanceFollowRedirects(false);
            c.setRequestProperty("User-Agent",agent);c.setRequestProperty("x-hwid",id);c.setRequestProperty("x-device-os","Android");c.setRequestProperty("x-device-model","Flyx Android");
            try{
                int status=c.getResponseCode();if(status>=300&&status<400){url=new URL(url,c.getHeaderField("Location"));continue;}
                if(status!=200)throw new IOException("Subscription HTTP "+status);
                JSONObject result=new JSONObject().put("body",new String(readLimited(c.getInputStream(),4*1024*1024),StandardCharsets.UTF_8));
                String title=c.getHeaderField("Profile-Title");if(title!=null){if(title.startsWith("base64:"))title=new String(android.util.Base64.decode(title.substring(7),android.util.Base64.DEFAULT),StandardCharsets.UTF_8);result.put("title",title);}
                String info=c.getHeaderField("Subscription-Userinfo");if(info!=null){long used=0;for(String part:info.split(";")){String[] pair=part.trim().split("=",2);if(pair.length!=2)continue;try{long n=Long.parseLong(pair[1].trim());if(pair[0].equals("upload")||pair[0].equals("download"))used+=n;else if(pair[0].equals("total")&&n>0)result.put("total",n);else if(pair[0].equals("expire")&&n>0)result.put("expire",n);}catch(NumberFormatException ignored){}}result.put("used",used);}
                return result;
            }finally{c.disconnect();}
        }
        throw new IOException("Too many redirects");
    }
}
