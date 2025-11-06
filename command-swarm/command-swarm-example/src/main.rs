mod behaviours;
use command_swarm::{
    BehaviourHandlerDispatcherTrait, SwarmHandler, SwarmLoopBuilder, make_command_swarm,
};
use libp2p::{Multiaddr, PeerId, noise, swarm::Swarm, tcp, yamux};
use std::time::Duration;

use crate::behaviours::echo::{EchoBehaviour, EchoBehaviourHandler, EchoCommand};
use crate::behaviours::ping::{PingBehaviourHandler, PingCommand};
/// Swarm-level commands (example in main.rs)
#[derive(Debug)]
pub enum SwarmLevelCommand {
    /// Establish connection with peer
    Dial { 
        peer_id: PeerId, 
        addr: Multiaddr,
        response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>
    },

    /// Start listening on specified address
    ListenOn { 
        addr: Multiaddr,
        response: tokio::sync::oneshot::Sender<Result<libp2p::Multiaddr, Box<dyn std::error::Error + Send + Sync>>>
    },
}

impl command_swarm::SwarmCommand for SwarmLevelCommand {
    type Output = ();
}

// Generation of top-level behaviour and combined commands
make_command_swarm! {
    behaviour_name: MyBehaviour,
    behaviours_handlers: {
        echo: EchoBehaviourHandler,
        ping: PingBehaviourHandler
    },
    commands: {
        name: MyCommands,
        swarm_level: SwarmLevelCommand
    },
    swarm_handler: MySwarmHandler
}

#[derive(Default)]
struct MySwarmHandler;

#[async_trait::async_trait]
impl SwarmHandler<MyBehaviour> for MySwarmHandler {
    type Command = SwarmLevelCommand;

    async fn handle_command(&mut self, swarm: &mut Swarm<MyBehaviour>, cmd: Self::Command) {
        match cmd {
            SwarmLevelCommand::ListenOn { addr, response } => {
                println!("🔄 [SwarmHandler] Starting to listen on address: {}", addr);
                let result = swarm.listen_on(addr.clone())
                    .map(|_| addr)
                    .map_err(|e| e.into());
                let _ = response.send(result);
            }
            SwarmLevelCommand::Dial { peer_id, addr, response } => {
                println!(
                    "📨 [SwarmHandler] Received Dial command - Peer: {:?}, Address: {}",
                    peer_id, addr
                );
                // For now, just acknowledge the command
                let _ = response.send(Ok(()));
            }
        }
    }

    async fn handle_event(
        &mut self,
        _swarm: &mut Swarm<MyBehaviour>,
        event: &libp2p::swarm::SwarmEvent<
            <MyBehaviour as libp2p::swarm::NetworkBehaviour>::ToSwarm,
        >,
    ) {
        match event {
            libp2p::swarm::SwarmEvent::Behaviour(behaviour_event) => match behaviour_event {
                MyBehaviourEvent::Echo(event) => {
                    println!("📡 [SwarmHandler] Echo event received: {:?}", event);
                }
                _ => {}
            },
            _ => {
                println!("🌐 [SwarmHandler] Swarm event: {:?}", event);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_thread_ids(true)
        .with_thread_names(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");

    println!("🚀 Application started");
    // Create Swarm using SwarmBuilder (correct pattern from examples)
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|_key| MyBehaviour {
            echo: EchoBehaviour::new(),
            ping: libp2p::ping::Behaviour::default(),
        })
        .unwrap()
        .build();


    let local_peer_id = swarm.local_peer_id().clone();
    println!("🆔 [Main] Local PeerId: {:?}", swarm.local_peer_id());

    let behaviour_handler_disptacher = MyBehaviourHandlerDispatcher {
        echo: EchoBehaviourHandler::default(),
        ping: PingBehaviourHandler::default(),
        swarm_handler: MySwarmHandler::default(),
    };

    let sl2_builder: SwarmLoopBuilder<MyBehaviour, MyBehaviourHandlerDispatcher, MyCommands> =
        SwarmLoopBuilder::new()
            .with_behaviour_handler(behaviour_handler_disptacher)
            .with_channel_size(32)
            .with_swarm(swarm);
    let (command_tx, stopper, swarm_loop) = sl2_builder.build().unwrap();

    // Start SwarmLoop in a separate task
    println!("🚀 [Main] Starting SwarmLoop in background task...");
    let swarm_handle = tokio::spawn(async move { swarm_loop.run().await });

    // Send commands AFTER starting the loop
    println!("📤 [Main] Sending ListenOn command with response...");
    let (listen_response_tx, listen_response_rx) = tokio::sync::oneshot::channel();
    command_tx
        .send(MyCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
            addr: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
            response: listen_response_tx,
        }))
        .await
        .unwrap();

    // Wait for listen command result
    match listen_response_rx.await {
        Ok(Ok(listen_addr)) => println!("✅ [Main] Successfully listening on: {}", listen_addr),
        Ok(Err(e)) => println!("❌ [Main] Failed to start listening: {}", e),
        Err(_) => println!("❌ [Main] Listen response channel closed"),
    }

    println!("📤 [Main] Sending Echo command with response...");
    let (echo_response_tx, echo_response_rx) = tokio::sync::oneshot::channel();
    command_tx
        .send(MyCommands::echo(EchoCommand::SendMessage {
            peer_id: local_peer_id.clone(),
            text: "Hello from Echo!".to_string(),
            response: echo_response_tx,
        }))
        .await
        .unwrap();

    // Wait for echo command result
    match echo_response_rx.await {
        Ok(Ok(())) => println!("✅ [Main] Echo command completed successfully"),
        Ok(Err(e)) => println!("❌ [Main] Echo command failed: {}", e),
        Err(_) => println!("❌ [Main] Echo response channel closed"),
    }

    println!("📤 [Main] Sending Ping command...");
    command_tx
        .send(MyCommands::ping(PingCommand::DummyTest))
        .await
        .unwrap();

    // Stop the SwarmLoop after 5 seconds using the stopper
    let stopper_clone = stopper.clone();
    tokio::spawn(async move {
        println!("⏰ [Main] Stopper task: waiting 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("🛑 [Main] Stopper task: sending shutdown signal...");
        stopper_clone.stop();
    });

    // Wait for SwarmLoop to finish gracefully
    println!("⏳ [Main] Waiting for SwarmLoop to finish...");
    swarm_handle.await.unwrap().unwrap();

    println!("✅ [Main] Program completed successfully");
}
