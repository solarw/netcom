#!/usr/bin/env python3
"""
P2P Echo Protocol Test

This script demonstrates a simple echo protocol between two P2P nodes:
1. Creates two nodes (server and client) in the same process
2. Client connects to server 
3. Client opens a stream to server, sends random data, and waits for a response
4. Server echoes back the data it receives
5. Client verifies the response matches the original data
6. The process repeats 5 times with a short pause between each round
"""

import asyncio
import random
import string
import logging
from p2p_network import Node, KeyPair, generate_keypair

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class EchoNode:
    """A P2P node that can act as both client and server in an echo protocol"""
    
    def __init__(self, name, kad_server=False):
        """Initialize the node with a name for logging purposes"""
        self.name = name
        self.key_pair = generate_keypair()
        self.node = Node(self.key_pair, False, kad_server)
        self._event_task = None
        self.running = False
    
    async def start(self, port):
        """Start the node and listen on the specified port"""
        await self.node.start()
        self.running = True
        
        # Start event processing loop
        self._event_task = asyncio.create_task(self._process_events())
        
        # Listen on the specified port
        address = await self.node.listen_port(port, "127.0.0.1")
        logger.info(f"Node {self.name} (ID: {self.node.peer_id()}) listening on {address}")
        
        # Bootstrap Kademlia DHT
        await self.node.bootstrap_kad()
        
        return address
    
    async def _process_events(self):
        """Process events from the node"""
        try:
            while self.running:
                event = await self.node.get_next_event(timeout_ms=100)
                if event is None:
                    await asyncio.sleep(0.01)
                    continue
                
                event_type = event.get('type')
                
                if event_type == 'PeerConnected':
                    peer_id = event.get('peer_id')
                    logger.info(f"Node {self.name}: Connected to {peer_id}")
                
                elif event_type == 'PeerDisconnected':
                    peer_id = event.get('peer_id')
                    logger.info(f"Node {self.name}: Disconnected from {peer_id}")
                
                elif event_type == 'IncomingStream':
                    stream = event.get('stream')
                    if stream:
                        asyncio.create_task(self._handle_stream(stream))
                
                elif event_type == 'AuthEvent':
                    auth_event = event.get('auth_event', {})
                    auth_type = auth_event.get('type')
                    
                    if auth_type == 'VerifyPorRequest':
                        # Auto-accept auth requests
                        connection_id = auth_event.get('connection_id')
                        peer_id = auth_event.get('peer_id')
                        await self.node.submit_por_verification(connection_id, True, {})
                        logger.debug(f"Node {self.name}: Accepted auth from {peer_id}")
                    
                    elif auth_type == 'MutualAuthSuccess':
                        peer_id = auth_event.get('peer_id')
                        logger.info(f"Node {self.name}: Mutual auth established with {peer_id}")
        
        except asyncio.CancelledError:
            logger.debug(f"Node {self.name}: Event loop cancelled")
        except Exception as e:
            logger.error(f"Node {self.name}: Error in event loop: {e}")
    
    async def _handle_stream(self, stream):
        """Handle an incoming stream - read data and echo it back"""
        try:
            peer_id = stream.peer_id
            
            # Read the data
            data = await stream.read_to_end()
            if data:
                message = data.decode()
                logger.info(f"Node {self.name}: Received from {peer_id}: '{message}'")
                
                # Echo the data back
                logger.info(f"Node {self.name}: Echoing back to {peer_id}")
                await stream.write(data)
                await stream.write_eof()
            
            # Close the stream
            await stream.close()
            logger.debug(f"Node {self.name}: Stream to {peer_id} closed")
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error handling stream: {e}")
    
    async def connect(self, address):
        """Connect to another node at the specified address"""
        connected = await self.node.connect(address)
        if connected:
            logger.info(f"Node {self.name}: Connected to {address}")
            return True
        else:
            logger.error(f"Node {self.name}: Failed to connect to {address}")
            return False
    
    async def send_echo(self, peer_id, data):
        """Send data to a peer and wait for an echo response"""
        try:
            # Open stream
            logger.info(f"Node {self.name}: Opening stream to {peer_id}")
            stream = await self.node.open_stream(peer_id)
            
            # Send data
            logger.info(f"Node {self.name}: Sending: '{data}'")
            await stream.write(data.encode())
            
            # Signal end of message
            logger.debug(f"Node {self.name}: Sending EOF")
            await stream.write_eof()
            
            # Read response
            logger.debug(f"Node {self.name}: Waiting for response")
            response_data = await stream.read_to_end()
            
            # Close stream
            await stream.close()
            
            if response_data:
                response = response_data.decode()
                logger.info(f"Node {self.name}: Received response: '{response}'")
                return response
            else:
                logger.warning(f"Node {self.name}: Received empty response")
                return None
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error in echo exchange: {e}")
            return None
    
    def peer_id(self):
        """Get the node's peer ID"""
        return self.node.peer_id()
    
    async def stop(self):
        """Stop the node and its event loop"""
        self.running = False
        
        if self._event_task:
            self._event_task.cancel()
            try:
                await self._event_task
            except asyncio.CancelledError:
                pass
        
        await self.node.stop()
        logger.info(f"Node {self.name}: Stopped")


async def generate_random_string(length=20):
    """Generate a random string of the specified length"""
    return ''.join(random.choice(string.ascii_letters + string.digits) for _ in range(length))


async def main():
    """Main entry point for the P2P echo test"""
    logger.info("Starting P2P Echo Protocol Test")
    
    # Create server and client nodes
    server = EchoNode("Server", kad_server=True)
    client = EchoNode("Client")
    
    try:
        # Start the nodes
        server_address = await server.start(port=8000)
        await client.start(port=8001)
        
        server_peer_id = server.peer_id()
        
        # Connect client to server
        logger.info("Connecting client to server...")
        if not await client.connect(server_address):
            logger.error("Failed to connect client to server")
            return
        
        # Wait for authentication to complete
        await asyncio.sleep(1)
        
        # Run 5 echo exchanges
        for i in range(5):
            logger.info(f"=== Echo Test Round {i+1} ===")
            
            # Generate random test data
            test_data = await generate_random_string(random.randint(20, 100))
            
            # Send message and get response
            response = await client.send_echo(server_peer_id, test_data)
            
            # Verify response matches original message
            if response == test_data:
                logger.info(f"✅ Round {i+1} PASSED: Response matches sent data")
            else:
                logger.error(f"❌ Round {i+1} FAILED: Response does not match!")
                logger.error(f"  Sent:     '{test_data}'")
                logger.error(f"  Received: '{response}'")
            
            # Sleep between rounds
            if i < 4:  # Don't sleep after the last round
                sleep_time = random.uniform(1.0, 3.0)
                logger.info(f"Sleeping for {sleep_time:.2f} seconds")
                await asyncio.sleep(sleep_time)
        
        logger.info("Echo test complete!")
    
    finally:
        # Stop the nodes
        await client.stop()
        await server.stop()
        logger.info("Test finished, all nodes stopped")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Test interrupted")
    except Exception as e:
        logger.error(f"Unexpected error: {e}")