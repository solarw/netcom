# tests/test_kad_discovery.py
import pytest
import time
from test_utils.p2p_node import Node

def test_kademlia_discovery():
    """Тест для проверки Kademlia обнаружения и обмена сообщениями между узлами."""
    
    # Создаем три узла: Kademlia-сервер и два клиента
    with Node(logfile_path="node1.log", kad_server_mode=True) as node1, \
         Node(logfile_path="node2.log") as node2, \
         Node(logfile_path="node3.log") as node3:
        
        # Проверяем что все узлы успешно запущены
        assert node1.peer_id is not None, "Node 1 failed to start"
        assert node2.peer_id is not None, "Node 2 failed to start"
        assert node3.peer_id is not None, "Node 3 failed to start"
        
        # Печатаем информацию об узлах
        print(f"Node 1 peer ID: {node1.peer_id}")
        print(f"Node 1 multiaddr: {node1.multiaddr}")
        print(f"Node 2 peer ID: {node2.peer_id}")
        print(f"Node 2 multiaddr: {node2.multiaddr}")
        print(f"Node 3 peer ID: {node3.peer_id}")
        print(f"Node 3 multiaddr: {node3.multiaddr}")
        
        # Соединяем node2 и node3 с node1 (Kademlia-сервером)
        assert node2.connect(node1.multiaddr), "Node 2 failed to connect to Node 1"
        assert node3.connect(node1.multiaddr), "Node 3 failed to connect to Node 1"
        
        # Даем больше времени на установление соединений и распространение информации в DHT
        time.sleep(10)
        
        # Node3 ищет Node2 через Kademlia
        assert node3.find(node2.peer_id), "Node 3 failed to find Node 2 via Kademlia"
        
        # После нахождения явно устанавливаем прямое соединение между Node3 и Node2
        assert node3.connect(node2.multiaddr), "Node 3 failed to connect directly to Node 2"
        
        # Даем время на установление прямого соединения
        time.sleep(5)
        
        # Отправляем сообщение от Node3 к Node2
        message = "hi from kad!"
        assert node3.stream_message(node2.peer_id, message, timeout=30), \
               "Node 3 failed to send message to Node 2"
        
        # Даем время на обработку и получение сообщения
        time.sleep(5)
        
        # Проверяем, получил ли Node2 сообщение от Node3
        assert node2.check_received_message(sender_peer_id=node3.peer_id, message=message, timeout=30), \
               "Node 2 did not receive message from Node 3"
        
        print("Kademlia discovery test successful!")