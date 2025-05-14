import uuid
import pytest
import asyncio
import time
from p2p_network import Node, KeyPair, PeerId

class NodeTool:
    """Вспомогательный класс для тестирования узлов p2p сети"""
    
    def __init__(self, peer_id=None, kad_server=False):
        """
        Инициализация узла
        
        :param peer_id: Идентификатор узла (если None, будет сгенерирован автоматически)
        :param kad_server: Флаг, является ли узел Kademlia сервером
        """
        # Создаем ключевую пару для узла
        self.key_pair = KeyPair()
        
        # Создаем Node с правильными параметрами в соответствии с API
        # Node принимает key_pair, enable_mdns (bool), kad_server_mode (bool)
        self.node = Node(self.key_pair, False, kad_server)
        
        self.received_messages = []
        self.connection_events = []
        self.find_events = []
        self.auth_events = []
        self.mutual_auth_success = []
        
        # Запускаем асинхронную задачу для обработки событий
        self._event_task = None
    
    async def start(self, port):
        """
        Запуск узла на указанном порту
        
        :param port: Порт для прослушивания
        :return: Адрес, на котором запущен узел
        """
        # Запускаем узел
        await self.node.start()
        
        # Запускаем обработку событий
        self._event_task = asyncio.create_task(self._process_events())
        
        # Настраиваем прослушивание на определенном порту
        address = await self.node.listen_port(port, "127.0.0.1")
        
        # Если это Kademlia сервер, включаем и bootstrap-им Kademlia
        await self.node.bootstrap_kad()
        
        return address
    
    async def _process_events(self):
        """Обрабатывает события от узла"""
        try:
            while True:
                # Получаем события с таймаутом 100 мс
                event = await self.node.get_next_event(timeout_ms=100)
                if event is None:
                    # Если нет событий, просто продолжаем цикл
                    await asyncio.sleep(0.1)
                    continue
                
                # Обрабатываем полученное событие
                event_type = event.get('type')
                
                if event_type == 'PeerConnected':
                    peer_id = event.get('peer_id')
                    self.connection_events.append((peer_id, True))
                    print(f"Node {self.node.peer_id()} connected to {peer_id}")
                
                elif event_type == 'PeerDisconnected':
                    peer_id = event.get('peer_id')
                    self.connection_events.append((peer_id, False))
                    print(f"Node {self.node.peer_id()} disconnected from {peer_id}")
                
                elif event_type == 'KadRoutingUpdated':
                    peer_id = event.get('peer_id')
                    self.find_events.append((peer_id, True))
                    print(f"Node {self.node.peer_id()} found {peer_id}")
                
                elif event_type == 'IncomingStream':
                    stream = event.get('stream')
                    if stream:
                        asyncio.create_task(self._handle_stream(stream))
                
                elif event_type == 'AuthEvent':
                    # Обработка события аутентификации
                    auth_event = event.get('auth_event', {})
                    auth_type = auth_event.get('type')
                    
                    if auth_type == 'VerifyPorRequest':
                        # Получаем данные о запросе аутентификации
                        connection_id = auth_event.get('connection_id')
                        peer_id = auth_event.get('peer_id')
                        
                        # Сохраняем событие аутентификации
                        self.auth_events.append((peer_id, connection_id, auth_type))
                        
                        print(f"Node {self.node.peer_id()} received PoR verification request from {peer_id} (conn_id: {connection_id})")
                        
                        # Автоматически подтверждаем аутентификацию
                        # Создаем метаданные для ответа
                        metadata = {'node_type': 'test_node'}
                        
                        # Подтверждаем аутентификацию
                        await self.node.submit_por_verification(connection_id, True, metadata)
                        print(f"Node {self.node.peer_id()} automatically accepted PoR from {peer_id}")
                    
                    elif auth_type == 'MutualAuthSuccess':
                        # Взаимная аутентификация успешно завершена
                        peer_id = auth_event.get('peer_id')
                        connection_id = auth_event.get('connection_id')
                        
                        # Сохраняем событие успешной взаимной аутентификации
                        self.mutual_auth_success.append((peer_id, connection_id))
                        
                        print(f"Node {self.node.peer_id()} completed mutual authentication with {peer_id} (conn_id: {connection_id})")
        
        except asyncio.CancelledError:
            # Задача была отменена
            pass
        except Exception as e:
            # Логируем ошибку и продолжаем
            print(f"Error processing events: {e}")
    
    async def _handle_stream(self, stream):
        """
        Обработчик входящего потока
        
        :param stream: Объект потока
        """
        try:
            # Получаем peer_id отправителя
            peer_id = stream.peer_id
            
            # Читаем сообщение из потока
            message = await stream.read_to_end()
            if message:
                decoded_message = message.decode()
                print(f"Node {self.node.peer_id()} received message from {peer_id}: {decoded_message}")
                self.received_messages.append((peer_id, decoded_message))
            
            # Закрываем поток после получения сообщения
            await stream.close()
        
        except Exception as e:
            print(f"Error handling stream: {e}")
    
    async def connect(self, address):
        """
        Подключение к другому узлу
        
        :param address: Адрес узла для подключения
        :return: True если подключение успешно, иначе False
        """
        # Выполняем подключение
        connected = await self.node.connect(address)
        
        if connected:
            print(f"Node {self.node.peer_id()} initiated connection to {address}")
            # Возвращаем успех подключения
            return True
        
        return False
    
    async def connect_and_wait_auth(self, address, peer_id, timeout=10):
        """
        Подключение к другому узлу с ожиданием успешной аутентификации
        
        :param address: Адрес узла для подключения
        :param peer_id: Идентификатор узла для проверки аутентификации
        :param timeout: Время ожидания в секундах
        :return: True если подключение и аутентификация успешны, иначе False
        """
        # Выполняем подключение
        connected = await self.node.connect(address)
        
        if connected:
            print(f"Node {self.node.peer_id()} initiated connection to {address}")
            
            # Ждем успешной взаимной аутентификации
            if await self.wait_for_mutual_auth_success(peer_id, timeout):
                print(f"Node {self.node.peer_id()} successfully authenticated with {peer_id}")
                return True
            else:
                print(f"Node {self.node.peer_id()} failed to authenticate with {peer_id} within {timeout} seconds")
                return False
        
        return False
    
    async def find_peer(self, peer_id):
        """
        Поиск узла по его идентификатору
        
        :param peer_id: Идентификатор искомого узла
        :return: Адрес найденного узла или None
        """
        # Запускаем поиск узла
        await self.node.find(peer_id)
        
        # Ждем немного, чтобы поиск успел выполниться
        await asyncio.sleep(1)
        
        # Получаем найденные адреса
        addresses = await self.node.search_peer_addresses(peer_id)
        
        # Возвращаем первый адрес, если есть
        return addresses[0] if addresses else None
    
    async def send_message(self, peer_id, message):
        """
        Отправка сообщения другому узлу
        
        :param peer_id: Идентификатор получателя
        :param message: Текст сообщения
        :return: True если сообщение успешно отправлено, иначе False
        """
        try:
            # Проверяем, есть ли успешная взаимная аутентификация с узлом
            if not self.is_mutually_authenticated(peer_id):
                print(f"Warning: Node {self.node.peer_id()} is not mutually authenticated with {peer_id}")
                print(f"Waiting for mutual authentication to complete...")
                
                # Ждем успешной взаимной аутентификации перед отправкой сообщения
                if not await self.wait_for_mutual_auth_success(peer_id, timeout=10):
                    print(f"Error: Mutual authentication with {peer_id} failed. Cannot send message.")
                    return False
            
            print(f"Opening stream to {peer_id}...")
            # Открываем поток к узлу
            stream = await self.node.open_stream(peer_id)
            
            print(f"Node {self.node.peer_id()} sending message to {peer_id}: {message}")
            # Отправляем сообщение
            await stream.write(message.encode())

            # Закрываем поток
            await stream.close()
            
            return True
        
        except Exception as e:
            print(f"Error sending message: {e}")
            return False
    
    def get_peer_id(self):
        """
        Получение идентификатора узла
        
        :return: Идентификатор узла
        """
        return self.node.peer_id()
    
    def is_connected_to(self, peer_id):
        """
        Проверка наличия соединения с узлом
        
        :param peer_id: Идентификатор узла
        :return: True если соединение установлено, иначе False
        """
        for event_peer_id, connected in self.connection_events:
            if str(event_peer_id) == str(peer_id) and connected:
                return True
        return False
    
    def was_peer_found(self, peer_id):
        """
        Проверка успешного поиска узла
        
        :param peer_id: Идентификатор искомого узла
        :return: True если узел был найден, иначе False
        """
        for event_peer_id, found in self.find_events:
            if str(event_peer_id) == str(peer_id) and found:
                return True
        return False
    
    def was_auth_requested(self, peer_id):
        """
        Проверка запроса аутентификации от узла
        
        :param peer_id: Идентификатор узла
        :return: True если был запрос аутентификации, иначе False
        """
        for event_peer_id, _, _ in self.auth_events:
            if str(event_peer_id) == str(peer_id):
                return True
        return False
    
    def is_mutually_authenticated(self, peer_id):
        """
        Проверка успешной взаимной аутентификации с узлом
        
        :param peer_id: Идентификатор узла
        :return: True если взаимная аутентификация успешна, иначе False
        """
        for event_peer_id, _ in self.mutual_auth_success:
            if str(event_peer_id) == str(peer_id):
                return True
        return False
    
    def has_received_message_from(self, peer_id):
        """
        Проверка получения сообщения от узла
        
        :param peer_id: Идентификатор узла
        :return: True если получено сообщение от узла, иначе False
        """
        for message_peer_id, _ in self.received_messages:
            if str(message_peer_id) == str(peer_id):
                return True
        return False
    
    def get_message_from(self, peer_id):
        """
        Получение сообщения от конкретного узла
        
        :param peer_id: Идентификатор узла
        :return: Текст сообщения или None если сообщений нет
        """
        for message_peer_id, message in self.received_messages:
            if str(message_peer_id) == str(peer_id):
                return message
        return None
    
    async def wait_for_connection(self, peer_id, timeout=5):
        """
        Ожидание установления соединения с узлом
        
        :param peer_id: Идентификатор узла
        :param timeout: Время ожидания в секундах
        :return: True если соединение установлено, иначе False
        """
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.is_connected_to(peer_id):
                return True
            await asyncio.sleep(0.1)
        return False
    
    async def wait_for_find(self, peer_id, timeout=5):
        """
        Ожидание успешного поиска узла
        
        :param peer_id: Идентификатор искомого узла
        :param timeout: Время ожидания в секундах
        :return: True если узел найден, иначе False
        """
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.was_peer_found(peer_id):
                return True
            await asyncio.sleep(0.1)
        return False
    
    async def wait_for_auth_request(self, peer_id, timeout=5):
        """
        Ожидание запроса аутентификации от узла
        
        :param peer_id: Идентификатор узла
        :param timeout: Время ожидания в секундах
        :return: True если был запрос аутентификации, иначе False
        """
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.was_auth_requested(peer_id):
                return True
            await asyncio.sleep(0.1)
        return False
    
    async def wait_for_mutual_auth_success(self, peer_id, timeout=5):
        """
        Ожидание успешной взаимной аутентификации с узлом
        
        :param peer_id: Идентификатор узла
        :param timeout: Время ожидания в секундах
        :return: True если взаимная аутентификация успешна, иначе False
        """
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.is_mutually_authenticated(peer_id):
                return True
            await asyncio.sleep(0.1)
        return False
    
    async def wait_for_message(self, peer_id, timeout=5):
        """
        Ожидание получения сообщения от узла
        
        :param peer_id: Идентификатор узла
        :param timeout: Время ожидания в секундах
        :return: True если получено сообщение от узла, иначе False
        """
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.has_received_message_from(peer_id):
                return True
            await asyncio.sleep(0.1)
        return False
    
    async def stop(self):
        """Остановка узла"""
        # Отменяем задачу обработки событий
        if self._event_task:
            self._event_task.cancel()
            try:
                await self._event_task
            except asyncio.CancelledError:
                pass
            
        # Останавливаем узел
        await self.node.stop()

@pytest.mark.asyncio
async def test_p2p_network():
    """
    Тест p2p сети с тремя узлами, где:
    1. Узел 1 выступает как Kademlia сервер
    2. Узлы 2 и 3 подключаются к узлу 1
    3. Узел 3 находит узел 2 через Kademlia и отправляет ему сообщение
    """
    # Создаем три узла
    node1 = NodeTool(kad_server=True)
    node2 = NodeTool()
    node3 = NodeTool()
    
    try:
        # Запускаем узлы на конкретных портах
        address1 = await node1.start(port=8000)
        await node2.start(port=8001)
        await node3.start(port=8002)
        
        print(f"Node 1 started at {address1}")
        print(f"Node 1 peer ID: {node1.get_peer_id()}")
        print(f"Node 2 peer ID: {node2.get_peer_id()}")
        print(f"Node 3 peer ID: {node3.get_peer_id()}")
        
        # Сохраняем ID узлов для последующего использования
        node1_peer_id = node1.get_peer_id()
        node2_peer_id = node2.get_peer_id()
        node3_peer_id = node3.get_peer_id()
        
        # Подключаем node2 и node3 к node1 (Kademlia-серверу) с ожиданием аутентификации
        print("Connecting node2 to node1...")
        assert await node2.connect_and_wait_auth(address1, node1_peer_id), "Node 2 не смог подключиться и аутентифицироваться с Node 1"
        
        print("Connecting node3 to node1...")
        assert await node3.connect_and_wait_auth(address1, node1_peer_id), "Node 3 не смог подключиться и аутентифицироваться с Node 1"
        
        # Проверяем по событиям, что подключения успешно установлены и аутентифицированы
        print("Checking if connections are established and authenticated...")
        assert node2.is_mutually_authenticated(node1_peer_id), "Node 2 не завершил взаимную аутентификацию с Node 1"
        assert node3.is_mutually_authenticated(node1_peer_id), "Node 3 не завершил взаимную аутентификацию с Node 1"
        
        # Node 3 ищет Node 2 через DHT
        print(f"Node 3 is searching for Node 2 (peer ID: {node2_peer_id})...")
        node2_address = await node3.find_peer(node2_peer_id)
        assert node2_address is not None, "Node 3 не смог найти Node 2 через DHT"
        print(f"Node 3 found Node 2 at address: {node2_address}")
        
        # Node 3 подключается к Node 2 по найденному адресу с ожиданием аутентификации
        print("Node 3 is connecting to Node 2...")
        assert await node3.connect_and_wait_auth(node2_address, node2_peer_id), "Node 3 не смог подключиться и аутентифицироваться с Node 2"
        
        # Проверяем успешную взаимную аутентификацию между node3 и node2
        await asyncio.sleep(0.1)
        assert node3.is_mutually_authenticated(node2_peer_id), "Node 3 не завершил взаимную аутентификацию с Node 2"
        assert node2.is_mutually_authenticated(node3_peer_id), "Node 2 не завершил взаимную аутентификацию с Node 3"
        
        # Создаем уникальное тестовое сообщение с UUID4
        unique_id = str(uuid.uuid4())
        test_message = f"Hello+{unique_id}"
        
        print(f"Node 3 is sending message '{test_message}' to Node 2...")
        assert await node3.send_message(node2_peer_id, test_message), "Node 3 не смог отправить сообщение Node 2"
        
        # Ждем, чтобы убедиться, что сообщение получено
        print("Waiting for Node 2 to receive the message...")
        assert await node2.wait_for_message(node3_peer_id, timeout=5), "Node 2 не получил сообщение от Node 3"
        
        # Проверяем содержимое полученного сообщения
        received_message = node2.get_message_from(node3_peer_id)
        assert received_message == test_message, f"Полученное сообщение '{received_message}' не соответствует отправленному '{test_message}'"
        
        print(f"Test passed: Node 2 received message '{received_message}' from Node 3")
        
    finally:
        # Останавливаем все узлы
        print("Stopping nodes...")
        await node1.stop()
        await node2.stop()
        await node3.stop()
        print("All nodes stopped")