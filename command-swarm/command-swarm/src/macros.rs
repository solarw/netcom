//! Macros for simplifying Swarm creation with handlers

/// Macro for creating complete enum of swarm commands with automatic generation
/// 
/// This macro generates an enum with struct-like variants that include response channels
/// for command results. Each variant automatically includes a `response` field with
/// a oneshot channel for receiving command execution results.
/// 
/// # Example
/// ```
/// use libp2p::PeerId;
/// 
/// command_swarm::swarm_commands! {
///     EchoCommand {
///         SendMessage(peer_id: PeerId, text: String) -> (),
///     }
/// }
/// ```
/// 
/// This generates:
/// ```rust
/// pub enum EchoCommand {
///     SendMessage {
///         peer_id: PeerId,
///         text: String,
///         response: tokio::sync::oneshot::Sender<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
///     },
/// }
/// ```
#[macro_export]
macro_rules! swarm_commands {
    (
        $enum_name:ident {
            $(
                $variant:ident($($field:ident : $ftype:ty),*) -> $output:ty
            ),* $(,)?
        }
    ) => {
        // Generate enum that combines all commands
         #[derive(Debug)]
        pub enum $enum_name {
            $(
                $variant {
                    $( $field : $ftype, )*
                    response: tokio::sync::oneshot::Sender<
                        Result<$output, Box<dyn std::error::Error + Send + Sync>>
                    >,
                },
            )*
        }
    };
}

/// Macro for creating complete Command-Swarm infrastructure
#[macro_export]
macro_rules! make_command_swarm {
    (
        behaviour_name: $behaviour_name:ident,
        behaviours_handlers: {
            $($field:ident: $handler_type:ty),* $(,)?
        },
        commands: {
            name: $commands_name:ident,
            swarm_level: $swarm_level_command:ty
        },
        swarm_handler: $swarm_handler_type:ty
    ) => {
        // Generate top-level NetworkBehaviour
        #[derive(libp2p::swarm::NetworkBehaviour)]
        pub struct $behaviour_name {
            $(
                pub $field: <$handler_type as $crate::handlers::BehaviourHandler>::Behaviour,
            )*
        }

        // Generate combined command enum
        #[derive(Debug)]
        pub enum $commands_name {
            $(
                $field(<$handler_type as $crate::handlers::BehaviourHandler>::Command),
            )*
            SwarmLevel($swarm_level_command),
        }

        // Automatically implement SwarmCommand for the generated command enum
        impl $crate::SwarmCommand for $commands_name {
            type Output = ();
        }

        // Generate TryFrom implementations for each behaviour command
        $(
            impl std::convert::TryFrom<$commands_name> for <$handler_type as $crate::handlers::BehaviourHandler>::Command {
                type Error = ();

                fn try_from(cmd: $commands_name) -> Result<Self, Self::Error> {
                    match cmd {
                        $commands_name::$field(inner_cmd) => Ok(inner_cmd),
                        _ => Err(()),
                    }
                }
            }
        )*

        // Generate From implementation for SwarmLevelCommand
        impl std::convert::From<$swarm_level_command> for $commands_name {
            fn from(cmd: $swarm_level_command) -> Self {
                $commands_name::SwarmLevel(cmd)
            }
        }

        // Generate TryFrom implementation for SwarmLevelCommand
        impl std::convert::TryFrom<$commands_name> for $swarm_level_command {
            type Error = ();

            fn try_from(cmd: $commands_name) -> Result<Self, Self::Error> {
                match cmd {
                    $commands_name::SwarmLevel(inner_cmd) => Ok(inner_cmd),
                    _ => Err(()),
                }
            }
        }

        // Generate From implementations for each behaviour command
        $(
            impl std::convert::From<<$handler_type as $crate::handlers::BehaviourHandler>::Command> for $commands_name {
                fn from(cmd: <$handler_type as $crate::handlers::BehaviourHandler>::Command) -> Self {
                    $commands_name::$field(cmd)
                }
            }
        )*

        // Embed code from generate_handyswarm_types_xhandler directly
        paste::item! {
            pub struct [< $behaviour_name HandlerDispatcher >] {
                pub swarm_handler: $swarm_handler_type,
                $(
                    pub $field: $handler_type,
                )*
            }
        }

        paste::item! {
            #[async_trait::async_trait]
            impl BehaviourHandlerDispatcherTrait<$behaviour_name, $commands_name> for [< $behaviour_name HandlerDispatcher >] {
                /// Handle commands for behaviour
                async fn handle_commands(&mut self, swarm: &mut libp2p::Swarm<$behaviour_name>, command: $commands_name) {
                    tracing::debug!(command = ?command, "{}Dispatcher: Processing command", stringify!($behaviour_name));

                    match command {
                        $(
                            $commands_name::$field(inner_cmd) => {
                                let behaviour = &mut swarm.behaviour_mut().$field;
                                use $crate::handlers::BehaviourHandler;
                                self.$field.handle_cmd(behaviour, inner_cmd).await;
                            }
                        )*
                        $commands_name::SwarmLevel(inner_cmd) => {
                            tracing::debug!(command = ?inner_cmd, "{}Dispatcher: Processing swarm-level command", stringify!($behaviour_name));
                            use $crate::handlers::SwarmHandler;
                            self.swarm_handler.handle_command(swarm, inner_cmd).await;
                        }
                    }
                }

                /// Handle swarm event for behaviour
                async fn handle_swarm_event(&mut self, swarm: &mut libp2p::Swarm<$behaviour_name>, event: libp2p::swarm::SwarmEvent<<$behaviour_name as libp2p::swarm::NetworkBehaviour>::ToSwarm>) {
                    use $crate::handlers::SwarmHandler;
                    // Pass ALL events entirely to swarm_handler cause later swarm_handle can pass event
                    self.swarm_handler.handle_event(swarm, &event).await;

                    // Pprocess Behaviour events
                    if let libp2p::swarm::SwarmEvent::Behaviour(behaviour_event) = event {
                        tracing::debug!("{}Dispatcher: Unpacking SwarmEvent::Behaviour", stringify!($behaviour_name));
                        self.handle_events(swarm, behaviour_event).await;
                    }

                }

                /// Handle behaviour events
                async fn handle_events(&mut self, swarm: &mut libp2p::Swarm<$behaviour_name>, event: <$behaviour_name as libp2p::swarm::NetworkBehaviour>::ToSwarm) {
                    tracing::debug!("{}Dispatcher: Processing behaviour event", stringify!($behaviour_name));

                    // Use type alias to work around qualified paths issue
                    type Event = <$behaviour_name as libp2p::swarm::NetworkBehaviour>::ToSwarm;

                    // Use match with explicit enumeration of variants
                    match event {
                        $(
                            Event::[< $field:camel >](inner_event) => {
                                let behaviour = &mut swarm.behaviour_mut().$field;
                                use $crate::handlers::BehaviourHandler;
                                self.$field.handle_event(behaviour, &inner_event).await;
                            }
                        )*
                    }
                }
            }
        }


    };



}
