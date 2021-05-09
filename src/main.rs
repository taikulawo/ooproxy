use std::{
    io::{
        self,
        // https://stackoverflow.com/questions/25273816/why-do-i-need-to-import-a-trait-to-use-the-methods-it-defines-for-a-type
        // 注释掉 Write trait 试下
        Write,
    },
    net::{IpAddr, SocketAddr},
    sync::Arc,
    u16,
};

use clap::{load_yaml, App, AppSettings};
use ooproxy::{client::Client, config::Config, stream::{BiPipe, StreamWithBuffer}};
use tokio::net::{TcpListener, TcpStream};

use log::{error, info, warn, LevelFilter};
use backtrace::Backtrace;
#[tokio::main]
async fn main() {
    let yaml = load_yaml!("./cli.yaml");
    let app = clap::App::from_yaml(&yaml)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .get_matches();
    // enable log!
    let mut logger = env_logger::Builder::new();
    let log_level: &str = app.value_of("log-level").expect("we need log level");
    logger
        .filter(None, log_level.parse().expect("unknown log level"))
        .filter_module("tokio_net", LevelFilter::Warn)
        .target(env_logger::Target::Stdout)
        .format(|buf, r| {
            if r.level().as_str().to_uppercase() == "ERROR" {
                let bt = Backtrace::new();
                return writeln!(
                    buf,
                    "[{}] {}:{} {} {:?}",
                    r.level(),
                    r.file().unwrap_or("unknown"),
                    r.line().unwrap_or(0),
                    r.args(),
                    bt
                );
            }
            writeln!(
                buf,
                "[{}] {}:{} {}",
                r.level(),
                r.file().unwrap_or("unknown"),
                r.line().unwrap_or(0),
                r.args()
            )
        })
        .init();
    // info! 等需要放到 logger 之后，否则不会输出
    // info!("111");
    let host: IpAddr = app
        .value_of("host")
        .expect("missing host")
        .parse()
        .expect("invalid address");

    let port: usize = app
        .value_of("port")
        .expect("missing port")
        .parse()
        .expect("invalid port number");
    let socks_proxy_server: SocketAddr = app
        .value_of("socks5")
        .expect("socks5 server address missing")
        .parse()
        .expect("invalid socket address");
    let config = Arc::new(Config {
        socks5_server: socks_proxy_server,
        host,
        port,
    });
    // start listening
    let mut addr = SocketAddr::new(host, port as u16);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind port");
    info!("listen on {}", addr);
    while let Ok((socks, addr)) = listener.accept().await {
        let result = handle_client(socks, config.clone()).await;
        if let Err(err) = result {
            error!("handle_client error {}", err);
        }
    }
}

async fn handle_client(peer_left: TcpStream, config: Arc<Config>) -> io::Result<()> {
    let mut client = Client::from_socket(peer_left, config).await?;
    let remote = if client.dest.port == 443 {
        // https
        // try parse server name from TLS server_name extension
        client = client.retrive_dest().await?;
        client.connect_remote_server().await?
    } else {
        client.connect_remote_server().await?
    };
    client.do_pipe(remote).await?;
    Ok(())
}
