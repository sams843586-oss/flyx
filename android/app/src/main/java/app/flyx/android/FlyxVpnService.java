package app.flyx.android;

import android.app.*;
import android.content.*;
import android.net.VpnService;
import android.os.ParcelFileDescriptor;
import org.json.*;
import hev.htproxy.TProxyService;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;

public class FlyxVpnService extends VpnService {
    interface Completion { void complete(JSONObject result); }
    private static volatile FlyxVpnService instance;
    private static volatile Completion pending;
    private static volatile String state="idle",server=null,error=null;
    private static volatile long since=0;
    private static final Deque<String> logs=new ArrayDeque<>();
    private final ExecutorService worker=Executors.newSingleThreadExecutor();
    private final ScheduledExecutorService monitor=Executors.newSingleThreadScheduledExecutor();
    private Process singbox,xray;
    private ParcelFileDescriptor tun;
    private File run;
    private static final String STOP="app.flyx.STOP";

    static void connect(Context context,JSONObject args,Completion completion){
        pending=completion;
        try{context.startForegroundService(new Intent(context,FlyxVpnService.class).putExtra("config",args.toString()));}
        catch(Exception e){error=e.toString();state="failed";pending=null;completion.complete(snapshot());}
    }
    static void disconnect(Context context,Completion completion){
        FlyxVpnService service=instance;
        if(service==null){state="idle";error=null;completion.complete(snapshot());return;}
        service.worker.execute(()->{service.cleanup();state="idle";error=null;completion.complete(snapshot());service.stopSelf();});
    }
    static JSONObject snapshot(){
        JSONObject out=new JSONObject();
        try{
            out.put("ok",!state.equals("failed")).put("state",state).put("server",server==null?JSONObject.NULL:server)
                .put("since",since==0?JSONObject.NULL:since).put("error",error==null?JSONObject.NULL:error).put("version","0.1.0");
            synchronized(logs){out.put("logs",new JSONArray(logs));}
        }catch(JSONException ignored){}
        return out;
    }
    private static void log(String line){synchronized(logs){while(logs.size()>=300)logs.removeFirst();logs.addLast(line.length()>2000?line.substring(0,2000):line);}}
    @Override public void onCreate(){super.onCreate();instance=this;run=new File(getNoBackupFilesDir(),"vpn-run");run.mkdirs();
        getSystemService(NotificationManager.class).createNotificationChannel(new NotificationChannel("flyx-vpn","Flyx VPN",NotificationManager.IMPORTANCE_LOW));
        monitor.scheduleAtFixedRate(()->{if(state.equals("connected"))worker.execute(()->{
            if(state.equals("connected")&&(singbox==null||!singbox.isAlive()||(xray!=null&&!xray.isAlive())||!TProxyService.TProxyIsRunning()))fail(new IOException("VPN core stopped. Open connection logs."));
        });},1,1,TimeUnit.SECONDS);
    }
    private Notification notification(String name){
        PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE|PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stop=PendingIntent.getService(this,1,new Intent(this,FlyxVpnService.class).setAction(STOP),PendingIntent.FLAG_IMMUTABLE|PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Builder(this,"flyx-vpn").setSmallIcon(app.flyx.android.R.drawable.flyx).setContentTitle("Flyx VPN")
            .setContentText(name).setContentIntent(open).setOngoing(true).addAction(new Notification.Action.Builder(null,"Disconnect",stop).build()).build();
    }
    @Override public int onStartCommand(Intent intent,int flags,int id){
        startForeground(1,notification("Flyx"));
        if(intent==null){stopSelf();return START_NOT_STICKY;}
        if(STOP.equals(intent.getAction())){disconnect(this,result->{});return START_NOT_STICKY;}
        String config=intent.getStringExtra("config");Completion callback=pending;pending=null;
        worker.execute(()->{
            try{connectNow(new JSONObject(config));}
            catch(Exception e){fail(e);}
            if(callback!=null)callback.complete(snapshot());
        });
        return START_NOT_STICKY;
    }
    private void connectNow(JSONObject args)throws Exception{
        cleanup();error=null;state="idle";synchronized(logs){logs.clear();}
        JSONObject plan=args.getJSONObject("plan"),request=args.getJSONObject("request");
        server=plan.optString("name","Flyx");
        if(!plan.isNull("xray")){
            File config=save("xray.json",plan.getJSONObject("xray").toString());
            xray=startCore("libflyx_xray.so",config,"xray");
            int port=plan.getJSONObject("xray").getJSONArray("inbounds").getJSONObject(0).getInt("port");waitPort(port,xray);
        }
        singbox=startCore("libflyx_singbox.so",save("sing-box.json",plan.getJSONObject("singbox").toString()),"sing-box");
        int port=plan.getInt("proxyPort");waitPort(port,singbox);
        Builder builder=new Builder().setSession("Flyx").setMtu(1500).addAddress("172.19.0.1",30).addAddress("fdfe:dcba:9876::1",126)
            .addRoute("0.0.0.0",0).addRoute("::",0).addDnsServer("1.1.1.1").setBlocking(false);
        String split=request.optString("split","all");JSONArray apps=request.optJSONArray("apps");
        if(split.equals("only")&&apps!=null&&apps.length()>0){
            for(int i=0;i<apps.length();i++){String pkg=apps.getString(i);if(pkg.equals(getPackageName()))throw new IOException("Flyx cannot route its own core through itself");builder.addAllowedApplication(pkg);}
        }else{
            builder.addDisallowedApplication(getPackageName());
            if(split.equals("except")&&apps!=null)for(int i=0;i<apps.length();i++){String pkg=apps.getString(i);if(!pkg.equals(getPackageName()))builder.addDisallowedApplication(pkg);}
        }
        tun=builder.establish();if(tun==null)throw new IOException("VPN permission was revoked");
        String yaml="tunnel:\n  mtu: 1500\n  ipv4: 172.19.0.1\n  ipv6: 'fdfe:dcba:9876::1'\nsocks5:\n  address: 127.0.0.1\n  port: "+port+"\n  udp: 'udp'\nmisc:\n  log-level: warn\n";
        File tunnelConfig=save("tunnel.yml",yaml);
        if(!TProxyService.TProxyStartService(tunnelConfig.getAbsolutePath(),tun.getFd()))throw new IOException("Could not start Android tunnel");
        since=System.currentTimeMillis()/1000;state="connected";
        getSystemService(NotificationManager.class).notify(1,notification(server));
        log("VPN tunnel started");
    }
    private Process startCore(String binary,File config,String prefix)throws IOException{
        File executable=new File(getApplicationInfo().nativeLibraryDir,binary);
        Process process=new ProcessBuilder(executable.getAbsolutePath(),"run","-c",config.getAbsolutePath()).directory(run).redirectErrorStream(true).start();
        new Thread(()->{try(BufferedReader reader=new BufferedReader(new InputStreamReader(process.getInputStream(),StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null)log("["+prefix+"] "+line.replaceAll("\\u001B\\[[;\\d]*m",""));}catch(IOException ignored){}},"flyx-"+prefix+"-logs").start();
        return process;
    }
    private File save(String name,String content)throws IOException{File path=new File(run,name);try(FileOutputStream out=new FileOutputStream(path)){out.write(content.getBytes(StandardCharsets.UTF_8));}return path;}
    private void waitPort(int port,Process process)throws Exception{
        for(int i=0;i<100;i++){
            if(!process.isAlive())throw new IOException("VPN core exited: "+process.exitValue());
            try(java.net.Socket socket=new java.net.Socket()){socket.connect(new InetSocketAddress("127.0.0.1",port),100);return;}catch(IOException ignored){Thread.sleep(50);}
        }throw new IOException("VPN core startup timed out");
    }
    private void stopProcess(Process process){if(process==null)return;process.destroy();try{if(!process.waitFor(1,TimeUnit.SECONDS))process.destroyForcibly();}catch(InterruptedException e){process.destroyForcibly();Thread.currentThread().interrupt();}}
    private void cleanup(){
        try{TProxyService.TProxyStopService();}catch(UnsatisfiedLinkError ignored){}
        if(tun!=null){try{tun.close();}catch(IOException ignored){}tun=null;}
        stopProcess(singbox);singbox=null;stopProcess(xray);xray=null;
        if(run!=null)for(String name:new String[]{"sing-box.json","xray.json","tunnel.yml"})new File(run,name).delete();
        since=0;
    }
    private void fail(Exception e){log(e.toString());cleanup();error=e.getMessage();state="failed";stopForeground(STOP_FOREGROUND_REMOVE);stopSelf();}
    @Override public void onRevoke(){disconnect(this,result->{});}
    @Override public void onDestroy(){monitor.shutdownNow();worker.shutdownNow();cleanup();instance=null;if(!state.equals("failed"))state="idle";super.onDestroy();}
}
