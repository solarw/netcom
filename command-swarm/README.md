# Command-Swarm Workspace

A Rust workspace for building libp2p applications with command-based swarm management.

## Overview

This workspace contains two main components:

- **command-swarm**: A library providing traits and utilities for managing libp2p swarms through command-based interfaces
- **command-swarm-example**: Example application demonstrating how to use the command-swarm library

## Quick Start

### Prerequisites

- Rust 1.70+
- Cargo

### Building

```bash
# Build all packages
cargo build

# Run the example
cargo run -p command-swarm-example
```

### Testing

```bash
# Run tests for all packages
cargo test

# Check compilation
cargo check
```

## Project Structure

```
├── command-swarm/          # Library crate
│   ├── src/
│   │   ├── handlers.rs     # Handler traits
│   │   ├── macros.rs       # Code generation macros
│   │   ├── swarm_loop.rs   # Swarm management loop
│   │   └── lib.rs          # Library exports
│   └── Cargo.toml
├── command-swarm-example/  # Example application
│   ├── src/
│   │   ├── behaviours/     # Example behaviours
│   │   │   ├── echo/       # Echo protocol behaviour
│   │   │   └── ping/       # Ping protocol behaviour
│   │   └── main.rs         # Example application
│   └── Cargo.toml
└── Cargo.toml             # Workspace configuration
```

## Features

- **Command-based swarm management**: Send commands to control swarm behavior
- **Unified macro system**: Generate complete swarm infrastructure with `make_command_swarm!`
- **Multiple behaviours**: Combine multiple libp2p behaviours in a single swarm
- **Event handling**: Handle swarm events and behaviour events through unified interface
- **Async support**: Full async/await support for all operations

## Getting Started Without Examples

You can start using Command-Swarm without examining the example project. Here's what you need to know:

### 1. **Basic Architecture**

Command-Swarm provides a command-based interface for managing libp2p swarms. The key components are:

- **BehaviourHandler**: Handle commands and events for specific behaviours
- **SwarmHandler**: Handle swarm-level operations and events  
- **make_command_swarm!**: Macro that generates all necessary infrastructure

### 2. **Minimal Setup**

```rust
use command_swarm::{make_command_swarm, BehaviourHandler, SwarmHandler, SwarmLoopBuilder};
use async_trait::async_trait;

// 1. Define your swarm-level commands
#[derive(Debug, Clone)]
pub enum SwarmLevelCommand {
    ListenOn { addr: Multiaddr },
    Dial { peer_id: PeerId, addr: Multiaddr },
}

// 2. Implement BehaviourHandler for each behaviour
#[derive(Default)]
pub struct MyBehaviourHandler;

#[async_trait]
impl BehaviourHandler for MyBehaviourHandler {
    type Behaviour = libp2p::ping::Behaviour;
    type Event = libp2p::ping::Event;
    type Command = (); // Or define custom commands

    async fn handle_cmd(&mut self, _behaviour: &mut Self::Behaviour, _cmd: Self::Command) {}
    
    async fn handle_event(&mut self, _behaviour: &mut Self::Behaviour, event: &Self::Event) {
        println!("Behaviour event: {:?}", event);
    }
}

// 3. Implement SwarmHandler
#[derive(Default)]
struct MySwarmHandler;

#[async_trait]
impl SwarmHandler<MyBehaviour> for MySwarmHandler {
    type Command = SwarmLevelCommand;

    async fn handle_command(&mut self, swarm: &mut Swarm<MyBehaviour>, cmd: Self::Command) {
        match cmd {
            SwarmLevelCommand::ListenOn { addr } => {
                swarm.listen_on(addr).ok();
            }
            SwarmLevelCommand::Dial { peer_id, addr } => {
                // Handle dialing
            }
        }
    }

    async fn handle_event(&mut self, _swarm: &mut Swarm<MyBehaviour>, event: &SwarmEvent<MyBehaviourEvent>) {
        println!("Swarm event: {:?}", event);
    }
}

// 4. Generate infrastructure
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
```

### 3. **Common Patterns**

**Adding Custom Behaviour Commands:**
```rust
#[derive(Debug, Clone)]
pub enum MyCustomCommand {
    SendMessage { peer_id: PeerId, text: String },
    RequestData { key: String },
}

// Then implement BehaviourHandler with MyCustomCommand as Command type
```

**Multiple Behaviours:**
```rust
make_command_swarm! {
    behaviour_name: MyBehaviour,
    behaviours_handlers: {
        ping: PingHandler,
        identify: IdentifyHandler,
        kademlia: KademliaHandler
    },
    // ... rest of configuration
}
```

**Error Handling:**
```rust
async fn handle_command(&mut self, swarm: &mut Swarm<MyBehaviour>, cmd: Self::Command) {
    match cmd {
        SwarmLevelCommand::ListenOn { addr } => {
            if let Err(e) = swarm.listen_on(addr.clone()) {
                eprintln!("Failed to listen on {}: {}", addr, e);
            }
        }
        // ... other commands
    }
}
```

### 4. **Next Steps**

- Check `command-swarm/README.md` for detailed API documentation
- The library handles all the complex type generation and routing
- You only need to implement the handler traits for your specific needs
- All libp2p behaviours are supported out of the box

The library is designed to be intuitive - if you understand libp2p behaviours, you'll understand Command-Swarm.

## License

[Add your license here]
