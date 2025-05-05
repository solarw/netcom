use network::{commander::Commander, node::NetworkNode, utils::make_new_key};

mod network;
use tracing_subscriber::{fmt, util::SubscriberInitExt, EnvFilter};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        //.with_file(true)
        //.with_line_number(true)
        .with_max_level(tracing::Level::INFO)
        .init();

    let local_key = make_new_key();
    let (mut node, cmd_tx, event_rx, peer_id) = NetworkNode::new(local_key).await?;

    println!("Peer is is {}", node.local_peer_id());
    let node_task = tokio::spawn(async move {
        node.run().await;
    });
    let port = 5000;

    let cmd = Commander::new(cmd_tx);
    cmd.listen_port(port).await?;
    let _ = tokio::join!(node_task);
    Ok(())
}
