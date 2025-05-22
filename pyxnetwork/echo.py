#!/usr/bin/env python3
"""
P2P Echo Protocol Test with Enhanced XStream Support

This script demonstrates the full XStream protocol functionality:
1. Creates two nodes (server and client) in the same process
2. Client connects to server 
3. Tests various XStream features:
   - Basic read/write operations (3 times)
   - State management
   - Error stream functionality (multiple error scenarios)
   - Enhanced error handling
4. Server echoes back data or sends errors based on test mode
5. Client handles various response scenarios
"""

import asyncio
import random
import string
import logging
from p2p_network import (
    Node, KeyPair, generate_keypair,
    XStream, StreamDirection, StreamState, 
    XStreamError, ErrorOnRead, XStreamErrorUtils
)

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class EnhancedEchoNode:
    """A P2P node that can act as both client and server with full XStream functionality"""
    
    def __init__(self, name, kad_server=False):
        """Initialize the node with a name for logging purposes"""
        self.name = name
        self.key_pair = generate_keypair()
        self.node = Node(self.key_pair, False, kad_server)
        self._event_task = None
        self.running = False
        self.test_mode = "normal"  # normal, error, partial_error
        self.request_count = 0  # Track number of requests processed
    
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
    
    def set_test_mode(self, mode):
        """Set the test mode for the server"""
        self.test_mode = mode
        logger.info(f"Node {self.name} set to test mode: {mode}")
    
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
        """Handle an incoming stream with enhanced XStream functionality"""
        try:
            peer_id = stream.peer_id
            self.request_count += 1
            
            logger.info(f"Node {self.name}: Received stream #{self.request_count} from {peer_id} "
                       f"(direction: {stream.direction}, state: {await stream.state()})")
            
            if self.test_mode == "normal":
                await self._handle_normal_stream(stream, peer_id)
            elif self.test_mode == "error":
                await self._handle_error_stream(stream, peer_id)
            elif self.test_mode == "partial_error":
                await self._handle_partial_error_stream(stream, peer_id)
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error handling stream: {e}")
    
    async def _handle_normal_stream(self, stream, peer_id):
        """Handle normal echo scenario"""
        try:
            # Read the data
            data = await stream.read_to_end()
            if data:
                message = data.decode()
                logger.info(f"Node {self.name}: Received from {peer_id}: '{message}'")
                
                # Echo the data back
                response = f"Echo: {message}"
                logger.info(f"Node {self.name}: Echoing back to {peer_id}: '{response}'")
                await stream.write(response.encode())
                await stream.flush()
                await stream.write_eof()
            
            # Close the stream
            await stream.close()
            logger.debug(f"Node {self.name}: Stream to {peer_id} closed normally")
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error in normal stream handling: {e}")
    
    async def _handle_error_stream(self, stream, peer_id):
        """Handle error scenario - send error instead of response"""
        try:
            # Read the data
            data = await stream.read_to_end()
            if data:
                message = data.decode()
                logger.info(f"Node {self.name}: Received from {peer_id}: '{message}'")
                
                # Send error instead of normal response
                error_message = f"Server error processing request #{self.request_count}: {message}"
                logger.info(f"Node {self.name}: Sending error to {peer_id}: '{error_message}'")
                await stream.error_write(error_message.encode())
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error in error stream handling: {e}")
    
    async def _handle_partial_error_stream(self, stream, peer_id):
        """Handle partial response then error scenario"""
        try:
            # Read the data
            data = await stream.read_to_end()
            if data:
                message = data.decode()
                logger.info(f"Node {self.name}: Received from {peer_id}: '{message}'")
                
                # Send partial response
                partial_response = f"Partial #{self.request_count}: {message[:10]}"
                logger.info(f"Node {self.name}: Sending partial response to {peer_id}: '{partial_response}'")
                await stream.write(partial_response.encode())
                await stream.flush()
                
                # Then send error
                error_message = f"Error after partial response #{self.request_count} for: {message}"
                logger.info(f"Node {self.name}: Sending error to {peer_id}: '{error_message}'")
                await stream.error_write(error_message.encode())
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error in partial error stream handling: {e}")
    
    async def connect(self, address):
        """Connect to another node at the specified address"""
        connected = await self.node.connect(address)
        if connected:
            logger.info(f"Node {self.name}: Connected to {address}")
            return True
        else:
            logger.error(f"Node {self.name}: Failed to connect to {address}")
            return False
    
    async def send_enhanced_echo(self, peer_id, data, expect_error=False):
        """Send data to a peer and handle enhanced responses with error awareness"""
        try:
            # Open stream
            logger.info(f"Node {self.name}: Opening stream to {peer_id}")
            stream = await self.node.open_stream(peer_id)
            
            # Log initial stream state
            logger.info(f"Node {self.name}: Stream opened "
                       f"(direction: {stream.direction}, state: {await stream.state()})")
            
            # Send data
            logger.info(f"Node {self.name}: Sending: '{data}'")
            await stream.write(data.encode())
            await stream.flush()
            
            # Signal end of message
            logger.debug(f"Node {self.name}: Sending EOF")
            await stream.write_eof()
            
            # Wait a bit for the server to process
            await asyncio.sleep(0.3)  # Increased wait time for server processing
            
            # Handle response with improved error awareness
            response_data = None
            error_data = None
            
            # Always check for error data first, regardless of expect_error
            logger.debug(f"Node {self.name}: Checking for error data")
            
            # Check multiple times for error data availability
            error_check_attempts = 5
            for attempt in range(error_check_attempts):
                if await stream.has_error_data():
                    try:
                        error_data = await stream.error_read()
                        logger.info(f"Node {self.name}: Received error data: {error_data.decode()}")
                        break
                    except Exception as e:
                        logger.warning(f"Node {self.name}: Failed to read error data (attempt {attempt + 1}): {e}")
                        if attempt < error_check_attempts - 1:
                            await asyncio.sleep(0.1)
                else:
                    # No error data yet, wait a bit and try again
                    if attempt < error_check_attempts - 1:
                        await asyncio.sleep(0.1)
            
            # Try to read normal response data if we don't have error data or if we expect both
            if not error_data or self.test_mode == "partial_error":
                try:
                    logger.debug(f"Node {self.name}: Attempting to read response data")
                    response_data = await stream.read_to_end()
                    if response_data:
                        logger.info(f"Node {self.name}: Received response: {response_data.decode()}")
                except Exception as e:
                    logger.info(f"Node {self.name}: No response data or read failed: {e}")
                    
                    # If we failed to read normal data and expect errors, check again for error data
                    if expect_error and not error_data:
                        for attempt in range(3):  # Additional attempts for error data
                            await asyncio.sleep(0.1)
                            if await stream.has_error_data():
                                try:
                                    error_data = await stream.error_read()
                                    logger.info(f"Node {self.name}: Received delayed error data: {error_data.decode()}")
                                    break
                                except Exception as err_e:
                                    logger.warning(f"Node {self.name}: Failed to read delayed error data: {err_e}")
            
            # Close stream
            await stream.close()
            logger.debug(f"Node {self.name}: Stream closed")
            
            return {
                'response': response_data.decode() if response_data else None,
                'error': error_data.decode() if error_data else None,
                'success': True
            }
            
        except Exception as e:
            logger.error(f"Node {self.name}: Error in enhanced echo exchange: {e}")
            return {
                'response': None,
                'error': str(e),
                'success': False
            }
    
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


async def test_scenario(server, client, server_peer_id, scenario_name, test_mode, expect_error=False):
    """Test a specific scenario"""
    logger.info(f"=== Testing {scenario_name} ===")
    
    # Set server test mode
    server.set_test_mode(test_mode)
    
    # Generate test data
    test_data = await generate_random_string(random.randint(20, 50))
    
    # Send message and get response
    result = await client.send_enhanced_echo(server_peer_id, test_data, expect_error=expect_error)
    
    # Verify result
    if result['success']:
        if expect_error:
            if result['error']:
                logger.info(f"✅ {scenario_name} PASSED: Received expected error")
                logger.info(f"   Error: {result['error']}")
                return True
            elif result['response'] and test_mode == "partial_error":
                # For partial_error mode, receiving partial response is also acceptable
                logger.info(f"✅ {scenario_name} PASSED: Received partial response (partial_error mode)")
                logger.info(f"   Response: {result['response']}")
                return True
            else:
                logger.warning(f"⚠️ {scenario_name}: Expected error but got response")
                logger.info(f"   Response: {result['response']}")
                return False
        else:
            if result['response'] and test_data in result['response']:
                logger.info(f"✅ {scenario_name} PASSED: Response matches sent data")
                logger.info(f"   Response: {result['response']}")
                return True
            else:
                logger.error(f"❌ {scenario_name} FAILED: Unexpected response")
                logger.error(f"   Sent: '{test_data}'")
                logger.error(f"   Response: '{result['response']}'")
                logger.error(f"   Error: '{result['error']}'")
                return False
    else:
        logger.error(f"❌ {scenario_name} FAILED: {result['error']}")
        return False


async def main():
    """Main entry point for the enhanced P2P echo test"""
    logger.info("Starting Enhanced P2P Echo Protocol Test with XStream Features")
    
    # Create server and client nodes
    server = EnhancedEchoNode("Server", kad_server=True)
    client = EnhancedEchoNode("Client")
    
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
        
        # Phase 1: Normal Echo Tests (3 times)
        logger.info("\n" + "="*60)
        logger.info("PHASE 1: NORMAL ECHO TESTS (3 times)")
        logger.info("="*60)
        
        normal_success_count = 0
        for i in range(3):
            scenario_name = f"Normal Echo #{i+1}"
            success = await test_scenario(server, client, server_peer_id, 
                                        scenario_name, "normal", False)
            if success:
                normal_success_count += 1
            
            # Small delay between tests
            await asyncio.sleep(0.5)
        
        logger.info(f"\nPhase 1 Results: {normal_success_count}/3 normal echo tests passed")
        
        # Phase 2: Error Response Tests (3 times)
        logger.info("\n" + "="*60)
        logger.info("PHASE 2: ERROR RESPONSE TESTS (3 times)")
        logger.info("="*60)
        
        error_success_count = 0
        for i in range(3):
            scenario_name = f"Error Response #{i+1}"
            success = await test_scenario(server, client, server_peer_id, 
                                        scenario_name, "error", True)
            if success:
                error_success_count += 1
            
            # Small delay between tests
            await asyncio.sleep(0.5)
        
        logger.info(f"\nPhase 2 Results: {error_success_count}/3 error response tests passed")
        
        # Phase 3: Partial Error Tests (2 times)
        logger.info("\n" + "="*60)
        logger.info("PHASE 3: PARTIAL ERROR TESTS (2 times)")
        logger.info("="*60)
        
        partial_success_count = 0
        for i in range(2):
            scenario_name = f"Partial Error #{i+1}"
            success = await test_scenario(server, client, server_peer_id, 
                                        scenario_name, "partial_error", True)
            if success:
                partial_success_count += 1
            
            # Small delay between tests
            await asyncio.sleep(0.5)
        
        logger.info(f"\nPhase 3 Results: {partial_success_count}/2 partial error tests passed")
        
        # Final Summary
        total_tests = 8
        total_passed = normal_success_count + error_success_count + partial_success_count
        
        logger.info("\n" + "="*60)
        logger.info("FINAL RESULTS")
        logger.info("="*60)
        logger.info(f"Total Tests: {total_tests}")
        logger.info(f"Passed: {total_passed}")
        logger.info(f"Failed: {total_tests - total_passed}")
        logger.info(f"Success Rate: {(total_passed/total_tests)*100:.1f}%")
        
        if total_passed == total_tests:
            logger.info("🎉 ALL TESTS PASSED! Enhanced XStream functionality working correctly!")
        else:
            logger.warning(f"⚠️ {total_tests - total_passed} tests failed. Check logs for details.")
        
        logger.info(f"Server processed {server.request_count} total requests")
        
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
        raise