use std::{
    net::{IpAddr},
    sync::Arc,
};

use async_trait::async_trait;
use tokio::runtime::Builder;
use trust_dns_resolver::{
    config::*, error::ResolveError, TokioAsyncResolver,
    TokioHandle,
};
#[async_trait]
pub trait AsyncResolver {
    async fn resolve();
}

type ResolverResult<T> = Result<T, ResolveError>;

fn create_dns_resolver() -> ResolverResult<TokioAsyncResolver> {
    TokioAsyncResolver::new(
        ResolverConfig::default(),
        ResolverOpts::default(),
        TokioHandle,
    )
}

struct DNSServer {
    resolver: TokioAsyncResolver,
}

impl DNSServer {
    pub fn new() -> Option<DNSServer> {
        let resolver = match create_dns_resolver() {
            Ok(r) => r,
            Err(e) => return None,
        };
        Some(DNSServer { resolver })
    }
    pub async fn lookup<T>(&self, host: T) -> Option<IpAddr>
    where
        T: AsRef<str>,
    {
        match self.resolver.lookup_ip(host.as_ref()).await {
            Ok(ip) => {
                for x in ip.into_iter() {
                    if x.is_ipv4() || x.is_ipv6() {
                        return Some(x);
                    }
                }
                None
            }
            Err(_) => return None,
        }
    }
}

fn main() {
    let mut builder = Builder::new_multi_thread();
    let runtime = builder
        .enable_all()
        .build()
        .expect("failed to enable all tokio features");
    let dns_server = Arc::new(DNSServer::new());
    runtime.block_on(async move {
        let dns_server = dns_server.clone();
       if let Some(server) = &*dns_server {
           let result = server.lookup("www.baidu.com").await;
           println!("{:?}", result);
       }
    });
}
