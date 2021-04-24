use std::{io::{
        self,
        // https://stackoverflow.com/questions/25273816/why-do-i-need-to-import-a-trait-to-use-the-methods-it-defines-for-a-type
        // 注释掉 Write trait 试下
        Write
    }, net::{
        IpAddr,
        SocketAddr
    }, sync::Arc, u16};

use ooproxy::{
    client::Client,
    config::Config
};
use tokio::net::{TcpListener, TcpStream};
use clap::{
    load_yaml,
    App,
    AppSettings
};

use log:: {
    LevelFilter,
    info,
    error,
    warn
};

#[tokio::main]
async fn main() {
    let yaml = load_yaml!("./cli.yaml");
    let app = clap::App::from_yaml(&yaml)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .get_matches();
    // enable log!
    let mut logger = env_logger::Builder::new();
    
    logger
        .filter(None, "info".parse().expect("unknown log level"))
        .filter_module("tokio_net", LevelFilter::Warn)
        .target(env_logger::Target::Stdout)
        .format(|buf, r | writeln!(buf, "[{}] {}", r.level(), r.args()))
        .init();
    // info! 等需要放到 logger 之后，否则不会输出
    // info!("111");
    let host: IpAddr = app.value_of("host")
        .expect("missing host")
        .parse()
        .expect("invalid address");

    let port: usize = app.value_of("port")
        .expect("missing port")
        .parse()
        .expect("invalid port number");
    let socks_proxy_server: SocketAddr = app.value_of("socks5")
        .expect("socks5 server address missing")
        .parse()
        .expect("invalid socket address");
    let config =  Arc::new(Config {
        socks5_server: socks_proxy_server,
        host,
        port
    });
    // start listening
    let mut addr = SocketAddr::new(host, port as u16);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind port");
    info!("listen on {}", addr);
    while let Ok((socks, addr)) = listener.accept().await {
        let result = handle_client(socks, config.clone()).await;
        if let Err(err) = result {
            info!("handle_client error {}", err);
        }
    }
}

async fn handle_client(peer_left: TcpStream, config: Arc<Config>) -> io::Result<()>{
    let client =  Client::from_socket(peer_left, config).await?;
    if client.dest.port == 443 {
        // https
        // try parse server name from TLS server_name extension
        client.retrive_dest().await?.connect_remote_server().await?;
    }else {
        client.connect_remote_server().await?;
    }
    Ok(())
}
