#!/usr/bin/env python3
# examples/async_node.py
import asyncio
import sys
import os
from p2p_network_py import Node, PeerId, generate_keypair

async def main():
    # Create a keypair
    keypair = generate_keypair()
    print(f"Generated keypair")
    
    # Create a node with the keypair
    node = Node(key_pair=keypair)
    
    # Start the node
    await node.start()
    print(f"Started node with peer ID: {node.peer_id}")
    
    # Listen on a random port
    addr = await node.listen_port(port=0)
    print(f"Listening on {addr}")
    
    # Bootstrap Kademlia
    await node.bootstrap_kad()
    print("Bootstrapped Kademlia DHT")
    
    # Process events for a while
    print("Waiting for events (press Ctrl+C to exit)...")
    try:
        while True:
            event = await node.get_next_event(timeout_ms=1000)
            if event:
                event_type = event.get("type", "Unknown")
                print(f"Received event: {event_type}")
                
                # Handle specific events
                if event_type == "PeerConnected":
                    peer_id = event.get("peer_id", "unknown")
                    print(f"Connected to peer: {peer_id}")
                    
                elif event_type == "IncomingStream":
                    stream = event.get("stream")
                    if stream:
                        peer_id = stream.peer_id
                        # Read from stream
                        data = await stream.read_to_end(timeout_ms=5000)
                        try:
                            message = data.decode('utf-8')
                            print(f"Message from {peer_id}: '{message.strip()}'")
                            
                            # Send a reply
                            reply = f"Echo: {message.strip()}"
                            await node.stream_message(peer_id, reply)
                        except UnicodeDecodeError:
                            print(f"Binary data from {peer_id}: {len(data)} bytes")
                        finally:
                            # Close the stream
                            await stream.close()
            
            # Small sleep to prevent high CPU usage
            await asyncio.sleep(0.1)
            
    except KeyboardInterrupt:
        print("\nExiting...")
    finally:
        # Stop the node
        await node.stop()
        print("Node stopped")

if __name__ == "__main__":
    asyncio.run(main())