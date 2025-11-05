//! SwarmLoop - main event and command processing loop using MyBehaviourHandler

use futures::StreamExt;
use libp2p::Swarm;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use std::error::Error;
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, instrument};

/// Trait for BehaviourHandlerDispatcher that defines processing methods
#[async_trait::async_trait]
pub trait BehaviourHandlerDispatcherTrait<B, C>
where
    B: NetworkBehaviour,
    C: Send + 'static,
{
    async fn handle_commands(&mut self, swarm: &mut Swarm<B>, command: C);
    async fn handle_swarm_event(&mut self, swarm: &mut Swarm<B>, event: SwarmEvent<B::ToSwarm>);
    async fn handle_events(&mut self, swarm: &mut Swarm<B>, event: B::ToSwarm);
}

/// Cloneable stopper for SwarmLoop
#[derive(Clone)]
pub struct SwarmLoopStopper {
    shutdown_tx: watch::Sender<bool>,
}

impl SwarmLoopStopper {
    /// Stops the SwarmLoop by sending shutdown signal
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
        info!("SwarmLoopStopper: Shutdown signal sent");
    }
}

/// Main Swarm event processing loop using MyBehaviourHandler
pub struct SwarmLoop<B, H, C>
where
    B: NetworkBehaviour,
    H: Send + 'static,
    C: Send + 'static,
{
    pub swarm: Swarm<B>,
    command_rx: mpsc::Receiver<C>,
    shutdown_rx: watch::Receiver<bool>,
    behaviour_handler: H,
}

impl<B, H, C> SwarmLoop<B, H, C>
where
    B: NetworkBehaviour + 'static,
    H: Send + 'static,
    C: Send + 'static,
    H: BehaviourHandlerDispatcherTrait<B, C>,
{
    /// Start the main loop
    #[instrument(name = "swarm_loop", skip(self))]
    pub async fn run(mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Main loop started");
        loop {
            tokio::select! {
                Some(cmd) = self.command_rx.recv() => {
                    debug!("Received command from channel");
                    self.handle_command(cmd).await;
                }
                event = self.swarm.select_next_some() => {
                    debug!("Received event from Swarm");
                    self.handle_swarm_event(event).await;
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        info!("Shutdown signal received");
                        break; // exit the loop
                    }
                }
            }
        }
        info!("Main loop finished gracefully");
        Ok(())
    }

    #[instrument(name = "handle_command", skip(self, cmd))]
    async fn handle_command(&mut self, cmd: C) {
        debug!(
            command_type = std::any::type_name::<C>(),
            "Received command"
        );

        // Pass command to behaviour_handler
        self.behaviour_handler
            .handle_commands(&mut self.swarm, cmd)
            .await;
    }

    #[instrument(name = "handle_swarm_event", skip(self, event))]
    async fn handle_swarm_event(&mut self, event: SwarmEvent<B::ToSwarm>) {
        debug!("Received Swarm event");

        // Pass event to behaviour_handler
        self.behaviour_handler
            .handle_swarm_event(&mut self.swarm, event)
            .await;
    }
}

/// Builder for SwarmLoop
pub struct SwarmLoopBuilder<B, H, C>
where
    B: NetworkBehaviour,
    H: Send + 'static,
    C: Send + 'static,
{
    swarm: Option<Swarm<B>>,
    behaviour_handler: Option<H>,
    channel_size: usize,
    _phantom: std::marker::PhantomData<C>,
}

impl<B, H, C> SwarmLoopBuilder<B, H, C>
where
    B: NetworkBehaviour + 'static,
    H: Send + 'static,
    C: Send + 'static,
{
    pub fn new() -> Self {
        Self {
            swarm: None,
            behaviour_handler: None,
            channel_size: 32, // default channel size
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_swarm(mut self, swarm: Swarm<B>) -> Self {
        self.swarm = Some(swarm);
        self
    }

    pub fn with_behaviour_handler(mut self, behaviour_handler: H) -> Self {
        self.behaviour_handler = Some(behaviour_handler);
        self
    }

    pub fn with_channel_size(mut self, channel_size: usize) -> Self {
        self.channel_size = channel_size;
        self
    }

    pub fn build(self) -> Result<(mpsc::Sender<C>, SwarmLoopStopper, SwarmLoop<B, H, C>), String> {
        let swarm = self.swarm.ok_or("Swarm not set")?;
        let behaviour_handler = self.behaviour_handler.ok_or("Behaviour handler not set")?;

        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(self.channel_size);
        
        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let swarm_loop = SwarmLoop {
            swarm,
            command_rx,
            shutdown_rx,
            behaviour_handler,
        };

        let stopper = SwarmLoopStopper { shutdown_tx };

        info!("SwarmLoopBuilder: Created SwarmLoop with stopper");
        Ok((command_tx, stopper, swarm_loop))
    }
}

impl<B, H, C> Default for SwarmLoopBuilder<B, H, C>
where
    B: NetworkBehaviour + 'static,
    H: Send + 'static,
    C: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
