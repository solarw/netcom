# Command-Swarm Example

A complete example application demonstrating how to use the Command-Swarm library.

## Overview

This example shows how to create a libp2p application with multiple behaviours using the Command-Swarm framework. It includes two behaviours:

- **Echo behaviour**: Custom protocol for sending and receiving messages
- **Ping behaviour**: Standard libp2p ping protocol

## Quick Start

### Prerequisites

- Rust 1.70+
- Cargo

### Running the Example

```bash
# Run the example
cargo run -p command-swarm-example
```

The example will:
1. Create a libp2p swarm with Echo and Ping behaviours
2. Start listening on a random local address
3. Send test commands to both behaviours
4. Run for 5 seconds and then exit

## Project Structure

```
src/
├── main.rs                 # Main application
└── behaviours/            # Behaviour implementations
    ├── mod.rs             # Behaviour exports
    ├── echo/              # Echo behaviour
    │   ├── mod.rs
    │   ├── behaviour.rs   # EchoBehaviour implementation
    │   ├── command.rs     # EchoCommand enum
    │   ├── event.rs       # EchoEvent enum
    │   └── handler.rs     # EchoBehaviourHandler
    └── ping/              # Ping behaviour
        ├── mod.rs
        ├── command.rs     # PingCommand enum
        └── handler.rs     # PingBehaviourHandler
```

## Complete Working Example

Here's the full working example that you can copy and run:

```rust
mod behaviours;
use command_swarm::{
    BehaviourHandlerDispatcherTrait, SwarmHandler, SwarmLoopBuilder, make_command_swarm,
};
use libp2p::{Multiaddr, PeerId, noise, swarm::Swarm, tcp, yamux};
use std::{error::Error, time::Duration};
use tokio::time;

use crate::behaviours::echo::{EchoBehaviour, EchoBehaviourHandler, EchoCommand};
use crate::behaviours::ping::{PingBehaviourHandler, PingCommand};

/// Swarm-level commands
#[derive(Debug, Clone)]
pub enum SwarmLevelCommand {
    /// Establish connection with peer
    Dial { peer_id: PeerId, addr: Multiaddr },
    /// Start listening on specified address
    ListenOn { addr: Multiaddr },
}

// Generate the complete swarm infrastructure
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
            SwarmLevelCommand::ListenOn { addr } => {
                println!("🔄 MySwarmHandler: listening on address {}", addr);
                match swarm.listen_on(addr.clone()) {
                    Ok(_) => println!("✅ Successfully started listening on {}", addr),
                    Err(e) => println!("❌ Error trying to listen on {}: {}", addr, e),
                }
            }
            SwarmLevelCommand::Dial { peer_id, addr } => {
                println!(
                    "📨 Received SWARM Dial command: peer_id={:?}, addr={}",
                    peer_id, addr
                );
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
            libp2p::swarm::SwarmEvent::Behaviour(_behaviour_event) => {}
            _ => {
                println!("Got swarm event: {:?}", event);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create Swarm using SwarmBuilder (correct pattern from examples)
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_key| MyBehaviour {
            echo: EchoBehaviour::new(),
            ping: libp2p::ping::Behaviour::default(),
        })?
        .build();

    println!("Local PeerId: {:?}", swarm.local_peer_id());

    let behaviour_handler_disptacher = MyBehaviourHandlerDispatcher {
        echo: EchoBehaviourHandler {},
        ping: PingBehaviourHandler {},
        swarm_handler: MySwarmHandler {},
    };

    let sl2_builder: SwarmLoopBuilder<MyBehaviour, MyBehaviourHandlerDispatcher, MyCommands> =
        SwarmLoopBuilder::new()
            .with_behaviour_handler(behaviour_handler_disptacher)
            .with_channel_size(32)
            .with_swarm(swarm);
    let (command_tx, swarm_loop) = sl2_builder.build()?;

    // Send test ListenOn command
    command_tx
        .send(MyCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
            addr: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
        }))
        .await
        .unwrap();

    // Send test Echo command
    command_tx
        .send(MyCommands::echo(EchoCommand::SendMessage {
            peer_id: swarm_loop.swarm.local_peer_id().clone(),
            text: "Hello from Echo!".to_string(),
        }))
        .await
        .unwrap();

    // Send test Ping command
    command_tx
        .send(MyCommands::ping(PingCommand::DummyTest))
        .await
        .unwrap();

    // Run SwarmLoop with 5 second timeout
    println!("🚀 Starting SwarmLoop for 5 seconds...");
    tokio::select! {
        _ = swarm_loop.run() => {
            println!("SwarmLoop finished by itself");
        }
        _ = time::sleep(Duration::from_secs(5)) => {
            println!("⏰ Time's up! Finishing program after 5 seconds");
        }
    }

    println!("✅ Program completed");
    Ok(())
}
```

### Echo Behaviour

The Echo behaviour demonstrates a custom protocol:

- **EchoCommand::SendMessage**: Send a message to a peer
- **EchoEvent::MessageReceived**: Handle incoming messages

### Ping Behaviour

The Ping behaviour uses the standard libp2p ping protocol:

- **PingCommand::DummyTest**: Test command that prints "hi from Ping:DummyTest"
- Automatically handles ping events from libp2p

## Behaviour Implementation Pattern

Each behaviour follows the same pattern:

1. **Command enum**: Define behaviour-specific commands
2. **Handler struct**: Implement `BehaviourHandler` trait
3. **Integration**: Add to `make_command_swarm!` macro

### Example: Creating a New Behaviour

```rust
// 1. Define commands
#[derive(Debug, Clone)]
pub enum MyBehaviourCommand {
    DoSomething { param: String },
}

// 2. Implement handler
#[derive(Default)]
pub struct MyBehaviourHandler;

#[async_trait]
impl BehaviourHandler for MyBehaviourHandler {
    type Behaviour = MyBehaviour;
    type Event = MyEvent;
    type Command = MyBehaviourCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        match cmd {
            MyBehaviourCommand::DoSomething { param } => {
                println!("Doing something with: {}", param);
            }
        }
    }

    async fn handle_event(&mut self, behaviour: &mut Self::Behaviour, event: &Self::Event) {
        // Handle behaviour events
    }
}

// 3. Add to make_command_swarm! macro
make_command_swarm! {
    behaviour_name: MyBehaviour,
    behaviours_handlers: {
        my_behaviour: MyBehaviourHandler,
        // ... other behaviours
    },
    // ... rest of configuration
}
```

## Testing

Run the example and observe the output:

```bash
cargo run -p command-swarm-example
```

You should see:
- Swarm creation and peer ID
- Listening address
- Echo command processing
- Ping command output: "hi from Ping:DummyTest"
- Various swarm events

## Extending the Example

To add more behaviours:

1. Create a new directory in `src/behaviours/`
2. Implement the command, handler, and behaviour
3. Add to `behaviours/mod.rs`
4. Update `make_command_swarm!` in `main.rs`

## License

[Add your license here]
