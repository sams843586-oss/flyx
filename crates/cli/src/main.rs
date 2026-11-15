//! Консольный прототип: ссылка или подписка → конфиги ядер → запуск туннеля.

use std::ffi::OsStr;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use flexconnect_core::engine;
use flexconnect_core::ipc::{self, ConnectRequest, Request, Response, SplitMode, State, TunnelMode};
use flexconnect_core::model::Node;
use flexconnect_core::singbox::{Mode, Options, Split};
use flexconnect_core::subscription::{parse_subscription, Subscription};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "flexconnect", version, about = "FlexConnect — прототип ядра")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Показать серверы из ссылки, файла или подписки
    List {
        /// Ссылка vless://…, адрес подписки https://… или путь к файлу
        input: String,
    },
    /// Сгенерировать конфиги ядер
    Config {
        #[command(flatten)]
        sel: Select,
        /// Куда записать; без флага — в консоль
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Проверить конфиги настоящими ядрами, не подключаясь
    Check {
        #[command(flatten)]
        sel: Select,
        #[command(flatten)]
        cores: Cores,
    },
    /// Подключиться; Ctrl+C — отключиться
    Run {
        #[command(flatten)]
        sel: Select,
        #[command(flatten)]
        cores: Cores,
    },
    /// Управлять туннелем через службу FlexConnect
    Svc {
        #[command(subcommand)]
        action: SvcCmd,
    },
}

#[derive(Subcommand)]
enum SvcCmd {
    /// Состояние туннеля
    Status,
    /// Подключиться через службу
    Connect {
        #[command(flatten)]
        sel: Select,
    },
    /// Отключиться
    Disconnect,
    /// Последние строки журнала ядер
    Logs {
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    /// Виртуальный адаптер, весь трафик системы (нужны права администратора)
    Tun,
    /// Локальный HTTP/SOCKS-прокси
    Proxy,
}

#[derive(Args)]
struct Select {
    /// Ссылка vless://…, адрес подписки https://… или путь к файлу
    input: String,
    /// Номер сервера из `list`
    #[arg(short = 'n', long, default_value_t = 1)]
    server: usize,
    #[arg(short, long, value_enum, default_value = "tun")]
    mode: ModeArg,
    /// Порт локального прокси для --mode proxy
    #[arg(long, default_value_t = 2080)]
    port: u16,
    /// Приложение мимо VPN, например steam.exe (можно несколько раз)
    #[arg(long = "direct-app", value_name = "EXE")]
    direct_apps: Vec<String>,
    /// Через VPN только это приложение (можно несколько раз)
    #[arg(long = "only-app", value_name = "EXE", conflicts_with = "direct_apps")]
    only_apps: Vec<String>,
    /// Домен с поддоменами мимо VPN, например ru (можно несколько раз)
    #[arg(long = "direct-domain", value_name = "DOMAIN")]
    direct_domains: Vec<String>,
    /// DNS для прямого трафика
    #[arg(long, default_value = "77.88.8.8")]
    direct_dns: String,
    /// DNS для трафика через VPN
    #[arg(long, default_value = "1.1.1.1")]
    remote_dns: String,
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[derive(Args)]
struct Cores {
    /// Путь к sing-box.exe; иначе FLEXCONNECT_SINGBOX, .\bin\sing-box.exe или PATH
    #[arg(long)]
    singbox: Option<PathBuf>,
    /// Путь к xray.exe; иначе FLEXCONNECT_XRAY, .\bin\xray.exe или PATH
    #[arg(long)]
    xray: Option<PathBuf>,
}

/// Готовые конфиги для выбранного сервера.
struct Plan {
    node: Node,
    singbox: Value,
    /// Заполнен, если сервер требует второго ядра.
    xray: Option<Value>,
}

fn load(input: &str) -> Result<Subscription> {
    let body = if input.starts_with("http://") || input.starts_with("https://") {
        let tls = native_tls::TlsConnector::new().context("не удалось инициализировать TLS")?;
        let device_id = flexconnect_core::device::subscription_id().map_err(|e|anyhow!(e))?;
        let fetch = |user_agent: &str| -> Result<String> { Ok(ureq::AgentBuilder::new()
            .tls_connector(std::sync::Arc::new(tls.clone()))
            .timeout(Duration::from_secs(20))
            .build()
            .get(input)
            .set("User-Agent", user_agent)
            .set("x-hwid", &device_id)
            .set("x-device-os", flexconnect_core::platform::OS_NAME)
            .set("x-device-model", "Flyx Desktop")
            .call()
            .context("не удалось скачать подписку")?
            .into_string()
            .context("подписка пришла не текстом")?) };
        let mut selected = None;
        for agent in [concat!("Flyx/", env!("CARGO_PKG_VERSION")), "v2rayNG/1.10.0", "Happ/3.3.6", "INCY/3.7.2"] {
            if let Ok(body) = fetch(agent) { if !parse_subscription(&body).nodes.is_empty() { selected = Some(body); break; } }
        }
        selected.ok_or_else(||anyhow!("провайдер не выдал поддерживаемые серверы в проверенных форматах"))?
    } else if !input.contains("://") && Path::new(input).is_file() {
        std::fs::read_to_string(input).with_context(|| format!("не удалось прочитать {input}"))?
    } else {
        input.to_string()
    };
    Ok(parse_subscription(&body))
}

fn print_skipped(sub: &Subscription) {
    for s in &sub.skipped {
        eprintln!(
            "  пропущена строка {} ({}): {}",
            s.line,
            s.scheme.as_deref().unwrap_or("?"),
            s.reason
        );
    }
}

fn list(input: &str) -> Result<()> {
    let sub = load(input)?;
    if sub.nodes.is_empty() {
        print_skipped(&sub);
        bail!("серверов не найдено");
    }
    for (i, n) in sub.nodes.iter().enumerate() {
        println!(
            "{:>3}. {:<11} {:<11} {:<8} {}:{}  {}{}",
            i + 1,
            n.proto_name(),
            n.transport_name(),
            n.security_name(),
            n.server,
            n.port,
            n.name,
            if n.needs_xray() { "  [через xray]" } else { "" }
        );
    }
    print_skipped(&sub);
    Ok(())
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

fn pick(sel: &Select) -> Result<Node> {
    let mut sub = load(&sel.input)?;
    if sub.nodes.is_empty() {
        print_skipped(&sub);
        bail!("серверов не найдено");
    }
    if sel.server == 0 || sel.server > sub.nodes.len() {
        bail!("сервера №{} нет: всего их {}", sel.server, sub.nodes.len());
    }
    Ok(sub.nodes.swap_remove(sel.server - 1))
}

fn plan(sel: &Select) -> Result<Plan> {
    let node = pick(sel)?;
    let split = if !sel.only_apps.is_empty() {
        Split::Only(sel.only_apps.clone())
    } else if !sel.direct_apps.is_empty() {
        Split::Except(sel.direct_apps.clone())
    } else {
        Split::All
    };
    let opts = Options {
        mode: match sel.mode {
            ModeArg::Tun => Mode::Tun,
            ModeArg::Proxy => Mode::Proxy { port: sel.port },
        },
        split,
        direct_suffixes: sel.direct_domains.clone(),
        direct_dns: sel.direct_dns.clone(),
        remote_dns: sel.remote_dns.clone(),
        log_level: sel.log_level.clone(),
        ipv6: flexconnect_core::platform::host_has_ipv6(),
    };

    let port = free_port().context("не нашлось свободного локального порта")?;
    let engine::Plan { singbox, xray } =
        engine::plan(&node, &opts, port).map_err(|e| anyhow!("{}: {e}", node.name))?;
    Ok(Plan { node, singbox, xray })
}

/// Один запрос к службе и её ответ.
fn ask_service(request: &Request) -> Result<Response> {
    use std::io::{BufRead, BufReader, Write};
    let mut pipe = ipc::open_client().context(
        "служба FlexConnect не отвечает: установите её или запустите `flexconnect-service run`",
    )?;
    let mut line = serde_json::to_string(request)?;
    line.push('\n');
    pipe.write_all(line.as_bytes())?;
    let mut reply = String::new();
    BufReader::new(pipe).read_line(&mut reply)?;
    serde_json::from_str(&reply).context("служба прислала непонятный ответ")
}

fn svc(action: SvcCmd) -> Result<()> {
    let request = match action {
        SvcCmd::Status => Request::Status,
        SvcCmd::Disconnect => Request::Disconnect,
        SvcCmd::Logs { lines } => Request::Logs { lines },
        SvcCmd::Connect { sel } => {
            let (split, apps) = if !sel.only_apps.is_empty() {
                (SplitMode::Only, sel.only_apps.clone())
            } else if !sel.direct_apps.is_empty() {
                (SplitMode::Except, sel.direct_apps.clone())
            } else {
                (SplitMode::All, Vec::new())
            };
            Request::Connect(ConnectRequest {
                node: engine::service_node(pick(&sel)?),
                mode: match sel.mode {
                    ModeArg::Tun => TunnelMode::Tun,
                    ModeArg::Proxy => TunnelMode::Proxy,
                },
                proxy_port: sel.port,
                split,
                apps,
                direct_domains: sel.direct_domains.clone(),
                direct_dns: Some(sel.direct_dns.clone()),
                remote_dns: Some(sel.remote_dns.clone()),
            })
        }
    };
    let response = ask_service(&request)?;
    for line in &response.logs {
        println!("{line}");
    }
    let state = match response.state {
        State::Idle => "отключено".to_string(),
        State::Connected => format!("подключено: {}", response.server.as_deref().unwrap_or("?")),
        State::Failed => "ошибка".to_string(),
    };
    eprintln!("Состояние: {state}");
    if let Some(error) = &response.error {
        eprintln!("Причина: {error}");
    }
    if !response.ok {
        bail!("служба не выполнила запрос");
    }
    Ok(())
}

fn find_bin(explicit: &Option<PathBuf>, env: &str, file: &str) -> PathBuf {
    if let Some(p) = explicit {
        return p.clone();
    }
    if let Some(p) = std::env::var_os(env) {
        return p.into();
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let candidates = [exe_dir.map(|d| d.join(file)), Some(PathBuf::from("bin").join(file))];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(file))
}

/// Рабочая папка: конфиги с ключами не должны лежать в проекте или в общем %TEMP%.
fn run_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    let dir = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir).join("FlexConnect/run");
    #[cfg(not(windows))]
    let dir = flexconnect_core::platform::data_dir().map_err(|e| anyhow!(e))?.join("run");
    flexconnect_core::platform::private_dir(&dir)?;
    Ok(dir)
}

/// Файлы конфигов на диске; удаляются при любом выходе, включая ошибки.
struct ConfigFiles {
    dir: PathBuf,
    singbox: PathBuf,
    xray: Option<PathBuf>,
}

impl ConfigFiles {
    fn write(plan: &Plan) -> Result<Self> {
        let dir = run_dir()?;
        let files = ConfigFiles {
            singbox: dir.join("sing-box.json"),
            xray: plan.xray.as_ref().map(|_| dir.join("xray.json")),
            dir,
        };
        flexconnect_core::platform::write_private(&files.singbox, &serde_json::to_vec_pretty(&plan.singbox)?)?;
        if let (Some(path), Some(config)) = (&files.xray, &plan.xray) {
            flexconnect_core::platform::write_private(path, &serde_json::to_vec_pretty(config)?)?;
        }
        Ok(files)
    }
}

impl Drop for ConfigFiles {
    fn drop(&mut self) {
        std::fs::remove_file(&self.singbox).ok();
        if let Some(path) = &self.xray {
            std::fs::remove_file(path).ok();
        }
    }
}

/// Второе ядро: гасится вместе с нами, что бы ни случилось.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn missing(core: &Path, file: &str, flag: &str) -> String {
    format!(
        "не удалось запустить {}: положите {file} в папку bin или укажите {flag}",
        core.display()
    )
}

fn singbox_cmd(bin: &Path, subcommand: &str, files: &ConfigFiles) -> Command {
    let mut cmd = Command::new(bin);
    cmd.arg(subcommand).arg("-c").arg(&files.singbox).arg("-D").arg(&files.dir);
    cmd
}

fn xray_cmd<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(bin: &Path, args: I, config: &Path) -> Command {
    let mut cmd = Command::new(bin);
    cmd.args(args).arg("-c").arg(config);
    cmd
}

fn check(sel: &Select, cores: &Cores) -> Result<()> {
    let plan = plan(sel)?;
    let files = ConfigFiles::write(&plan)?;

    if let Some(config) = &files.xray {
        let bin = find_bin(&cores.xray, "FLEXCONNECT_XRAY", flexconnect_core::platform::XRAY);
        let status = xray_cmd(&bin, ["run", "-test"], config)
            .status()
            .with_context(|| missing(&bin, flexconnect_core::platform::XRAY, "--xray"))?;
        if !status.success() {
            bail!("xray не принял конфиг ({status})");
        }
    }
    let bin = find_bin(&cores.singbox, "FLEXCONNECT_SINGBOX", flexconnect_core::platform::SINGBOX);
    let status = singbox_cmd(&bin, "check", &files)
        .status()
        .with_context(|| missing(&bin, flexconnect_core::platform::SINGBOX, "--singbox"))?;
    if !status.success() {
        bail!("sing-box не принял конфиг ({status})");
    }
    eprintln!(
        "Конфиг для «{}» прошёл проверку: {}.",
        plan.node.name,
        if plan.xray.is_some() { "xray и sing-box" } else { "sing-box" }
    );
    Ok(())
}

fn run(sel: &Select, cores: &Cores) -> Result<()> {
    let plan = plan(sel)?;
    let files = ConfigFiles::write(&plan)?;
    let node = &plan.node;

    eprintln!("Подключаюсь: {} ({}:{})", node.name, node.server, node.port);
    if let Some(reason) = node.xray_reason() {
        eprintln!("Серверу нужен {reason}: sing-box держит туннель, xray — связь с сервером.");
    }
    eprintln!("Ctrl+C — отключиться.");
    // Ctrl+C получают и ядра: sing-box сам снимает адаптер и маршруты,
    // а мы дожидаемся его завершения, а не умираем первыми.
    ctrlc::set_handler(|| {}).ok();

    let _xray = match &files.xray {
        Some(config) => {
            let bin = find_bin(&cores.xray, "FLEXCONNECT_XRAY", flexconnect_core::platform::XRAY);
            let child = xray_cmd(&bin, ["run"], config)
                .spawn()
                .with_context(|| missing(&bin, flexconnect_core::platform::XRAY, "--xray"))?;
            Some(ChildGuard(child))
        }
        None => None,
    };

    let bin = find_bin(&cores.singbox, "FLEXCONNECT_SINGBOX", flexconnect_core::platform::SINGBOX);
    let status = singbox_cmd(&bin, "run", &files)
        .status()
        .with_context(|| missing(&bin, flexconnect_core::platform::SINGBOX, "--singbox"))?;
    if !status.success() {
        if matches!(sel.mode, ModeArg::Tun) {
            eprintln!("Подсказка: режиму TUN нужна консоль, запущенная от имени администратора.");
        }
        bail!("sing-box завершился с ошибкой ({status})");
    }
    eprintln!("Отключено.");
    Ok(())
}

fn config(sel: &Select, out: Option<PathBuf>) -> Result<()> {
    let plan = plan(sel)?;
    let value = match &plan.xray {
        Some(xray) => {
            eprintln!("Серверу нужен xray: в выводе два конфига, «sing-box» и «xray».");
            json!({ "sing-box": plan.singbox, "xray": xray })
        }
        None => plan.singbox,
    };
    let text = serde_json::to_string_pretty(&value)?;
    match out {
        Some(path) => std::fs::write(&path, text)
            .with_context(|| format!("не удалось записать {}", path.display())),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::List { input } => list(&input),
        Cmd::Config { sel, out } => config(&sel, out),
        Cmd::Check { sel, cores } => check(&sel, &cores),
        Cmd::Run { sel, cores } => run(&sel, &cores),
        Cmd::Svc { action } => svc(action),
    }
}
