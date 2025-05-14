#!/usr/bin/env python3
"""
Interactive P2P Network Application

This script provides an interactive command-line interface to work with the p2p_network library,
similar to the functionality provided by the Rust main.rs implementation.
"""

import asyncio
import argparse
import logging
import signal
import sys
import socket
from prompt_toolkit import PromptSession
from prompt_toolkit.completion import WordCompleter
from prompt_toolkit.history import InMemoryHistory
from typing import Optional, List, Dict, Set, Tuple

# Import from the correct module name
from p2p_network_py import Node, PeerId, KeyPair, generate_keypair

# Set up logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s',
    handlers=[logging.StreamHandler()]
)
logger = logging.getLogger(__name__)

def get_free_port():
    """Get a free port on the local machine."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(('', 0))
        return s.getsockname()[1]

class InteractiveP2PApp:
    """Interactive application for working with P2P Network."""
    
    def __init__(self, args):
        """Initialize the application with command-line arguments."""
        self.args = args
        self.node = None
        self.running = False
        self.received_messages = []
        self.discovered_peers = set()  # Set of discovered peer IDs
        self.authenticated_peers = set()  # Set of authenticated peer IDs
        self.listen_addr = None  # Store the listen address
        
        # Set up command completion
        command_list = [
            'help', 'exit', 'quit', 'status', 'connect', 'disconnect', 
            'listen', 'bootstrap_kad', 'find', 'stream', 'peers'
        ]
        self.command_completer = WordCompleter(command_list)
        
    def show_command_line_info(self):
        """Display banner and connection information."""
        print("\nP2P Network Interactive Console")
        print("Type 'help' for available commands.")
        print("\n=========================================")
        

        peer_id = self.node.peer_id()
        # Check if node and peer ID are available
        print(f"Your Peer ID: {peer_id}")
                
        addr = self.listen_addr
        addr_parts = addr.split("/")
        if len(addr_parts) >= 5:
            host = addr_parts[2]
            port = addr_parts[4]
            print(f"Listening on: {host}:{port}")
        
        # Get the peer ID as a string
        print(f"Full connect address: {addr}/p2p/{peer_id}")
        
        print("=========================================\n")
        
    async def start(self):
        """Start the application and node."""
        try:
            logger.info("Starting P2P Network Interactive Application")
            
            # Generate a keypair or use an existing one
            key_pair = generate_keypair()
            logger.info(f"Generated keypair")
            
            # Create the node with renamed parameters
            self.node = Node(
                key_pair=key_pair,
                _enable_mdns=not self.args.disable_mdns,
                _kad_server_mode=self.args.kad_server
            )
            
            # Start the node
            peer_id_str = await self.node.start()
            logger.info(f"Started node with peer ID: {peer_id_str}")
            
            # Use the free port instead of specified port if args.port is 0
            port = self.args.port if self.args.port != 0 else get_free_port()
            host = self.args.host  # Use specified host
            
            addr = await self.node.listen_port(port=port, host=host)
            self.listen_addr = addr  # Store the listen address
            logger.info(f"Listening on {addr}")
            
            # Log the full connect address for users to copy
            logger.info(f"Connect address: {addr}/p2p/{peer_id_str}")
            
            # Bootstrap Kademlia if needed
            if self.args.kad_server:
                await self.node.bootstrap_kad()
                logger.info("Bootstrapped Kademlia DHT")
            
            # Display welcome message and connection info
            self.show_command_line_info()
            
            # Start the event processing loop in the background
            self.running = True
            asyncio.create_task(self.process_events())
            
            # Start the command loop
            await self.command_loop()
            
        except Exception as e:
            logger.error(f"Error starting application: {e}")
            raise
    
    async def process_events(self):
        """Process events from the node in the background."""
        while self.running:
            try:
                event = await self.node.get_next_event(timeout_ms=1000)
                if event:
                    await self.handle_event(event)
            except Exception as e:
                logger.error(f"Error processing events: {e}")
            
            # Small sleep to prevent high CPU usage
            await asyncio.sleep(0.1)
    
    async def handle_event(self, event):
        """Handle different types of network events."""
        event_type = event.get("type", "Unknown")
        logger.info(f"Received event: {event_type} {event}")
        
        if event_type == "PeerConnected":
            peer_id = event.get("peer_id")
            if peer_id:
                self.discovered_peers.add(peer_id)
                logger.info(f"Connected to peer: {peer_id}")
        
        elif event_type == "PeerDisconnected":
            peer_id = event.get("peer_id")
            if peer_id and peer_id in self.discovered_peers:
                self.discovered_peers.remove(peer_id)
                logger.info(f"Disconnected from peer: {peer_id}")
                
        elif event_type == "IncomingStream":
            stream = event.get("stream")
            if stream:
                peer_id = stream.peer_id
                # Read from stream
                try:
                    data = await stream.read_to_end(timeout_ms=5000)
                    try:
                        message = data.decode('utf-8')
                        logger.info(f"Message from {peer_id}: '{message.strip()}'")
                        
                        # Add to received messages list
                        self.received_messages.append({
                            "peer_id": peer_id,
                            "message": message.strip()
                        })
                        
                        # Send a reply
                        reply = f"Echo: {message.strip()}"
                        await self.node.stream_message(peer_id, reply)
                    except UnicodeDecodeError:
                        logger.info(f"Binary data from {peer_id}: {len(data)} bytes")
                    finally:
                        # Close the stream
                        await stream.close()
                except Exception as e:
                    logger.error(f"Error handling incoming stream: {e}")
        
        elif event_type == "KadAddressAdded":
            peer_id = event.get("peer_id")
            addr = event.get("address")
            #logger.info(f"Kademlia address added: {peer_id} at {addr}")
        
        elif event_type == "KadRoutingUpdated":
            peer_id = event.get("peer_id")
            addresses = event.get("addresses", [])
            logger.info(f"Kademlia routing updated for {peer_id}")
            for addr in addresses:
                #logger.info(f"  - Address: {addr}")
                pass
        
        elif event_type == "AuthEvent":
            auth_event = event.get("auth_event", {})
            auth_type = auth_event.get("type", "Unknown")
            
            if "MutualAuthSuccess" in auth_type:
                peer_id = auth_event.get("peer_id")
                if peer_id:
                    self.authenticated_peers.add(peer_id)
                    logger.info(f"✅ Mutual authentication successful with peer: {peer_id}")

            if "VerifyPorRequest" in auth_type:
                # do por check
            
            # Additional auth event handling could be added here
    
    async def command_loop(self):
        """Run the interactive command loop."""
        session = PromptSession(history=InMemoryHistory(), completer=self.command_completer)
        
        while self.running:
            try:
                user_input = await session.prompt_async("p2p> ")
                if not user_input.strip():
                    continue
                    
                command_parts = user_input.strip().split()
                command = command_parts[0].lower()
                
                if command in ('exit', 'quit'):
                    self.running = False
                    await self.node.stop()
                    logger.info("Exiting...")
                    break
                
                elif command == 'help':
                    self.show_help()
                
                elif command == 'status':
                    await self.show_status()
                
                elif command == 'connect':
                    if len(command_parts) < 2:
                        print("Usage: connect <multiaddr>")
                        continue
                        
                    addr = command_parts[1]
                    print(f"Connecting to {addr}...")
                    success = await self.node.connect(addr)
                    
                    if success:
                        print(f"Successfully connected to {addr}")
                    else:
                        print(f"Failed to connect to {addr}")
                
                elif command == 'disconnect':
                    if len(command_parts) < 2:
                        print("Usage: disconnect <peer_id>")
                        continue
                        
                    peer_id_str = command_parts[1]
                    try:
                        peer_id = PeerId(peer_id_str)
                        print(f"Disconnecting from {peer_id}...")
                        success = await self.node.disconnect(peer_id)
                        
                        if success:
                            print(f"Successfully disconnected from {peer_id}")
                        else:
                            print(f"Failed to disconnect from {peer_id}")
                    except Exception as e:
                        print(f"Error: {e}")
                
                elif command == 'listen':
                    port = 0  # Default is random port
                    host = "127.0.0.1"  # Default is localhost
                    
                    if len(command_parts) > 1:
                        try:
                            port = int(command_parts[1])
                        except ValueError:
                            print("Port must be a number")
                            continue
                    
                    if len(command_parts) > 2:
                        host = command_parts[2]
                    
                    print(f"Listening on {host}:{port}...")
                    addr = await self.node.listen_port(port=port, host=host)
                    self.listen_addr = addr  # Store the listen address
                    print(f"Listening address: {addr}")
                    
                    # Update the command line info
                    self.show_command_line_info()
                
                elif command == 'bootstrap_kad':
                    print("Bootstrapping Kademlia DHT...")
                    success = await self.node.bootstrap_kad()
                    
                    if success:
                        print("Successfully bootstrapped Kademlia DHT")
                    else:
                        print("Failed to bootstrap Kademlia DHT")
                
                elif command == 'find':
                    if len(command_parts) < 2:
                        print("Usage: find <peer_id>")
                        continue
                        
                    peer_id_str = command_parts[1]
                    try:
                        peer_id = PeerId(peer_id_str)
                        print(f"Searching for peer: {peer_id}...")
                        success = await self.node.find(peer_id)
                        
                        if success:
                            print(f"Found peer: {peer_id}")
                            
                            # Try to get addresses
                            addresses = await self.node.search_peer_addresses(peer_id)
                            if addresses:
                                print("Addresses:")
                                for i, addr in enumerate(addresses):
                                    print(f"  [{i+1}] {addr}")
                            else:
                                print("No addresses found")
                        else:
                            print(f"Could not find peer: {peer_id}")
                    except Exception as e:
                        print(f"Error: {e}")
                
                elif command == 'stream':
                    if len(command_parts) < 3:
                        print("Usage: stream <peer_id> <message>")
                        continue
                        
                    peer_id_str = command_parts[1]
                    message = ' '.join(command_parts[2:])
                    
                    try:
                        peer_id = PeerId(peer_id_str)
                        print(f"Sending message to {peer_id}: '{message}'")
                        success = await self.node.stream_message(peer_id, message)
                        
                        if success:
                            print(f"Message sent successfully to {peer_id}")
                        else:
                            print(f"Failed to send message to {peer_id}")
                    except Exception as e:
                        print(f"Error: {e}")
                
                elif command == 'peers':
                    print("\nDiscovered peers:")
                    if not self.discovered_peers:
                        print("  None")
                    else:
                        for peer in self.discovered_peers:
                            auth_status = "✅ Authenticated" if peer in self.authenticated_peers else "⚠️ Not authenticated"
                            print(f"  - {peer} [{auth_status}]")
                    print()
                
                else:
                    print(f"Unknown command: {command}")
                    print("Type 'help' for available commands.")
            
            except KeyboardInterrupt:
                print("\nUse 'exit' to quit")
            except Exception as e:
                logger.error(f"Error in command loop: {e}")
    
    def show_help(self):
        """Display available commands and their usage."""
        print("\nAvailable commands:")
        print("  help                     - Show this help message")
        print("  status                   - Show node status")
        print("  connect <multiaddr>      - Connect to a peer with the given multiaddress")
        print("  disconnect <peer_id>     - Disconnect from a peer")
        print("  listen [port] [host]     - Listen on the specified port and host (default: random port, all interfaces)")
        print("  bootstrap_kad            - Bootstrap the Kademlia DHT")
        print("  find <peer_id>           - Find a peer using Kademlia DHT")
        print("  stream <peer_id> <msg>   - Send a message to a peer")
        print("  peers                    - List discovered peers")
        print("  exit, quit               - Exit the application")
        print()
    
    async def show_status(self):
        """Display the current status of the node."""
        if not self.node:
            print("Node not started")
            return
            
        print("\nNode Status:")
        print(f"  Peer ID: {self.node.peer_id}")
        print(f"  Discovered Peers: {len(self.discovered_peers)}")
        print(f"  Authenticated Peers: {len(self.authenticated_peers)}")
        print(f"  Received Messages: {len(self.received_messages)}")
        
        if self.listen_addr:
            print(f"  Listening Address: {self.listen_addr}")
            
        print()

async def main():
    """Main entry point for the application."""
    # Parse command-line arguments
    parser = argparse.ArgumentParser(description="P2P Network Interactive Application")
    parser.add_argument("--port", type=int, default=0, help="Port to listen on (0 = random port)")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="Host to listen on (default: all interfaces)")
    parser.add_argument("--disable-mdns", action="store_true", help="Disable mDNS discovery")
    parser.add_argument("--accept-all-auth", action="store_true", help="Accept all authentication requests")
    parser.add_argument("--kad-server", action="store_true", help="Run in Kademlia server mode")
    args = parser.parse_args()
    
    # Create and start the application
    app = InteractiveP2PApp(args)
    
    # Set up signal handlers
    loop = asyncio.get_event_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, lambda: asyncio.create_task(shutdown(app)))
    
    try:
        await app.start()
    except Exception as e:
        logger.error(f"Application error: {e}")
    finally:
        if app.running:
            await shutdown(app)

async def shutdown(app):
    """Gracefully shutdown the application."""
    try:
        if app.running:
            app.running = False
            if app.node:
                # Make sure we don't crash during shutdown
                try:
                    await app.node.stop()
                    logger.info("Node stopped successfully")
                except Exception as e:
                    logger.error(f"Error during node shutdown: {e}")
            logger.info("Application shutdown complete")
            # Give event loop time to process final events
            await asyncio.sleep(0.5)
    except Exception as e:
        logger.error(f"Error during shutdown: {e}")
        
if __name__ == "__main__":
    try:
        # Set custom exception handler to prevent crash dumps
        def custom_exception_handler(loop, context):
            exception = context.get('exception', None)
            if exception:
                logger.error(f"Caught exception: {exception}")
            else:
                logger.error(f"Caught exception context: {context}")
            
        loop = asyncio.get_event_loop()
        loop.set_exception_handler(custom_exception_handler)
        
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nExiting...")
    except SystemExit:
        # Normal exit
        pass
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        sys.exit(1)