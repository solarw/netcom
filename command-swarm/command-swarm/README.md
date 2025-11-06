# Command-Swarm

A Rust library for building libp2p applications with command-based swarm management.

## Overview

Command-Swarm provides a framework for managing libp2p swarms through a command-based interface, making it easier to build complex peer-to-peer applications. The library offers traits, utilities, and macros to simplify swarm management and behaviour handling.

## Features

- **Command-based interface**: Send commands to control swarm behaviour with response channels
- **Unified macro system**: Generate complete swarm infrastructure with `make_command_swarm!`
- **Command generation macros**: Create command enums with `swarm_commands!` macro
- **Multiple behaviours**: Combine multiple libp2p behaviours in a single swarm
- **Event handling**: Handle swarm events and behaviour events through unified interface
- **Async support**: Full async/await support for all operations
- **Response channels**: Built-in oneshot channels for command results

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
command-swarm = { path = "../command-swarm" }
```

## Quick Start

Here's a complete example that shows how to create a simple libp2p application with Command-Swarm:

```rust
use async_trait::async_trait;
use command_swarm::{make_command_swarm, BehaviourHandler, SwarmHandler, SwarmLoopBuilder};
use libp2p::{Multiaddr, PeerId, Swarm, noise, tcp, yamux};
use std::error::Error;

// Define swarm-level commands
#[derive(Debug, Clone)]
pub enum SwarmLevelCommand {
    ListenOn { addr: Multiaddr },
    Dial { peer_id: PeerId, addr: Multiaddr },
}

// Define a simple behaviour handler
#[derive(Default)]
pub struct MyBehaviourHandler;

#[async_trait]
impl BehaviourHandler for MyBehaviourHandler {
    type Behaviour = libp2p::ping::Behaviour;
    type Event = libp2p::ping::Event;
    type Command = (); // No custom commands for this simple example

    async fn handle_cmd(&mut self, _behaviour: &mut Self::Behaviour, _cmd: Self::Command) {
        // Handle behaviour-specific commands
    }

    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        println!("📡 Ping event: {:?}", event);
    }
}

// Define swarm handler
#[derive(Default)]
struct MySwarmHandler;

#[async_trait]
impl SwarmHandler<MyBehaviour> for MySwarmHandler {
    type Command = SwarmLevelCommand;

    async fn handle_command(&mut self, swarm: &mut Swarm<MyBehaviour>, cmd: Self::Command) {
        match cmd {
            SwarmLevelCommand::ListenOn { addr } => {
                println!("🔄 Listening on address: {}", addr);
                if let Err(e) = swarm.listen_on(addr.clone()) {
                    println!("❌ Failed to listen on {}: {}", addr, e);
                }
            }
            SwarmLevelCommand::Dial { peer_id, addr } => {
                println!("📨 Dialing peer {} at {}", peer_id, addr);
            }
        }
    }

    async fn handle_event(&mut self, _swarm: &mut Swarm<MyBehaviour>, event: &libp2p::swarm::SwarmEvent<MyBehaviourEvent>) {
        println!("🌐 Swarm event: {:?}", event);
    }
}

// Generate the complete swarm infrastructure
make_command_swarm! {
    behaviour_name: MyBehaviour,
    behaviours_handlers: {
        ping: MyBehaviourHandler
    },
    commands: {
        name: MyCommands,
        swarm_level: SwarmLevelCommand
    },
    swarm_handler: MySwarmHandler
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create the swarm
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_key| MyBehaviour {
            ping: libp2p::ping::Behaviour::default(),
        })?
        .build();

    println!("Local PeerId: {:?}", swarm.local_peer_id());

    // Create handler dispatcher
    let behaviour_handler_dispatcher = MyBehaviourHandlerDispatcher {
        ping: MyBehaviourHandler {},
        swarm_handler: MySwarmHandler {},
    };

    // Build and run swarm loop
    let (command_tx, swarm_loop) = SwarmLoopBuilder::new()
        .with_behaviour_handler(behaviour_handler_dispatcher)
        .with_swarm(swarm)
        .build()?;

    // Send commands
    command_tx.send(MyCommands::SwarmLevel(SwarmLevelCommand::ListenOn {
        addr: "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
    })).await?;

    // Run for 10 seconds
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    
    println!("✅ Application completed");
    Ok(())
}
```

## Core Concepts

### BehaviourHandler Trait

Implement this trait for each behaviour you want to handle:

```rust
use async_trait::async_trait;
use command_swarm::BehaviourHandler;

#[async_trait]
impl BehaviourHandler for MyBehaviourHandler {
    type Behaviour = MyBehaviour;
    type Event = MyEvent;
    type Command = MyCommand;

    async fn handle_cmd(&mut self, behaviour: &mut Self::Behaviour, cmd: Self::Command) {
        // Handle behaviour-specific commands
    }

    async fn handle_event(&mut self, behaviour: &mut Self::Behaviour, event: &Self::Event) {
        // Handle behaviour-specific events
    }
}
```

### SwarmHandler Trait

Implement this trait for swarm-level operations:

```rust
use async_trait::async_trait;
use command_swarm::SwarmHandler;

#[async_trait]
impl SwarmHandler<MyBehaviour> for MySwarmHandler {
    type Command = SwarmLevelCommand;

    async fn handle_command(&mut self, swarm: &mut Swarm<MyBehaviour>, cmd: Self::Command) {
        // Handle swarm-level commands like listening or dialing
    }

    async fn handle_event(&mut self, swarm: &mut Swarm<MyBehaviour>, event: &SwarmEvent<...>) {
        // Handle all swarm events
    }
}
```

### make_command_swarm! Macro

The main macro that generates all necessary types and infrastructure:

```rust
make_command_swarm! {
    behaviour_name: MyBehaviour,           // Generated NetworkBehaviour
    behaviours_handlers: {                 // List of behaviour handlers
        behaviour1: Behaviour1Handler,
        behaviour2: Behaviour2Handler
    },
    commands: {                            // Command definitions
        name: MyCommands,                  // Generated command enum
        swarm_level: SwarmLevelCommand     // Swarm-level command type
    },
    swarm_handler: MySwarmHandler          // Swarm handler type
}
```

### swarm_commands! Macro

Create command enums with built-in response channels:

```rust
use libp2p::PeerId;

command_swarm::swarm_commands! {
    EchoCommand {
        SendMessage(peer_id: PeerId, text: String) -> (),
    }
}
```

This generates an enum with struct-like variants that include response channels:

```rust
pub enum EchoCommand {
    SendMessage {
        peer_id: PeerId,
        text: String,
        response: oneshot::Sender<Result<(), Box<dyn Error + Send + Sync>>>,
    },
}
```

Usage with response channels:

```rust
let (response_tx, response_rx) = tokio::sync::oneshot::channel();
let cmd = EchoCommand::SendMessage {
    peer_id: local_peer_id.clone(),
    text: "Hello".to_string(),
    response: response_tx,
};

// Send command and wait for result
match response_rx.await {
    Ok(Ok(())) => println!("Command completed successfully"),
    Ok(Err(e)) => println!("Command failed: {}", e),
    Err(_) => println!("Response channel closed"),
}
```

## Generated Types

The macro generates:

- **NetworkBehaviour**: Combined behaviour with all specified behaviours
- **Command enum**: Unified command type with variants for each behaviour and swarm-level commands
- **HandlerDispatcher**: Dispatcher that routes commands and events to appropriate handlers
- **From/TryFrom implementations**: For easy command conversion

## Example

See the `command-swarm-example` package for a complete working example with Echo and Ping behaviours.

## API Reference

### Traits

- `BehaviourHandler`: Handle behaviour-specific commands and events
- `SwarmHandler`: Handle swarm-level commands and events
- `BehaviourHandlerDispatcherTrait`: Internal trait for dispatching commands and events

### Macros

- `make_command_swarm!`: Generate complete swarm infrastructure

### Types

- `SwarmLoop`: Main swarm management loop
- `SwarmLoopBuilder`: Builder for creating swarm loops

## License

[Add your license here]
