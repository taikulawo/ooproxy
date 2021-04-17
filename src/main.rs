use std::{
    io,
    net::SocketAddr,
    
};

use ooproxy::client::Client;
use tokio::net::{TcpListener, TcpStream};
use clap::{
    load_yaml,
    App,
    AppSettings
};

use log:: {
    LevelFilter
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
        .filter(None, "info")
        .filter_module("tokio_net", LevelFilter::Warn)
        .target(env_logger::Target::Stdout)
        .format(|buf, r | writeln!(buf, "[{}] {}", r.level(), r.args()))
        .init();

    // start listening
    let mut addr = SocketAddr::new("0.0.0.0".parse().unwrap(),8080);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind port");
    while let Ok((socks, addr)) = listener.accept().await {
        handle_client(socks);
    }
}

async fn handle_client(peer_left: TcpStream) -> io::Result<()>{
    let client =  Client::from_socket(peer_left).await?;
    if client.dest.port == 443 {
        // https
        // try parse server name from TLS server_name extension
        client.retrive_dest().await?.connect_remote_server();
    }else {
        client.connect_remote_server();
    }
    Ok(())
}
