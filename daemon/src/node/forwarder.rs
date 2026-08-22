use anyhow::Result;
use tokio::net::{TcpListener, TcpStream};

#[allow(dead_code)]
pub struct LocalForwarder;

impl LocalForwarder {
    #[allow(dead_code)]
    pub async fn spawn_port_bridge(port: u16, target_host: &str) -> Result<()> {
        let bind_addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&bind_addr).await?;
        let target = target_host.to_string();

        tokio::spawn(async move {
            loop {
                let (mut inbound, _client_addr) = match listener.accept().await {
                    Ok(res) => res,
                    Err(_) => continue,
                };

                let target_addr = format!("{}:{}", target, port);
                tokio::spawn(async move {
                    if let Ok(mut outbound) = TcpStream::connect(&target_addr).await {
                        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                    }
                });
            }
        });

        Ok(())
    }
}
