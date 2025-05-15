import pytest
import asyncio
import uuid
from p2p_network import Node, KeyPair, PeerId

class TestSimpleNode:
    """Класс для тестирования обмена сообщениями в P2P сети"""
    
    def __init__(self, kad_server=False):
        """
        Инициализация тестового узла
        
        :param kad_server: Флаг, является ли узел Kademlia сервером
        """
        # Создаем ключевую пару и узел
        self.key_pair = KeyPair()
        self.node = Node(self.key_pair, False, kad_server)
        
        # Переменные для отслеживания событий
        self.connections = []
        self.received_messages = []
        self.auth_completed = []
        
        # Переменная для хранения задачи обработки событий
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
        
        # Запускаем bootstrap для Kademlia
        await self.node.bootstrap_kad()
        
        return address
    
    async def _process_events(self):
        """Обрабатывает события от узла"""
        try:
            while True:
                # Получаем события
                event = await self.node.get_next_event(timeout_ms=100)
                if event is None:
                    # Если нет событий, продолжаем цикл
                    await asyncio.sleep(0.1)
                    continue
                
                # Обрабатываем события в зависимости от типа
                event_type = event.get('type')
                
                if event_type == 'PeerConnected':
                    peer_id = event.get('peer_id')
                    self.connections.append((peer_id, True))
                    print(f"⚡ Узел {self.node.peer_id()} подключен к {peer_id}")
                
                elif event_type == 'PeerDisconnected':
                    peer_id = event.get('peer_id')
                    self.connections.append((peer_id, False))
                    print(f"⚡ Узел {self.node.peer_id()} отключен от {peer_id}")
                
                elif event_type == 'IncomingStream':
                    stream = event.get('stream')
                    if stream:
                        asyncio.create_task(self._handle_stream(stream))
                
                elif event_type == 'AuthEvent':
                    auth_event = event.get('auth_event', {})
                    auth_type = auth_event.get('type')
                    
                    if auth_type == 'VerifyPorRequest':
                        # Автоматически принимаем запрос аутентификации
                        connection_id = auth_event.get('connection_id')
                        peer_id = auth_event.get('peer_id')
                        
                        await self.node.submit_por_verification(connection_id, True, {})
                        print(f"🔐 Узел {self.node.peer_id()} принял запрос аутентификации от {peer_id}")
                    
                    elif auth_type == 'MutualAuthSuccess':
                        peer_id = auth_event.get('peer_id')
                        self.auth_completed.append(peer_id)
                        print(f"🔒 Узел {self.node.peer_id()} успешно завершил взаимную аутентификацию с {peer_id}")
        
        except asyncio.CancelledError:
            # Задача была отменена
            pass
        except Exception as e:
            print(f"❌ Ошибка при обработке событий: {e}")
    
    async def _handle_stream(self, stream):
        """
        Обработчик входящего потока
        
        :param stream: Объект потока
        """
        try:
            peer_id = stream.peer_id
            
            # Читаем сообщение из потока
            message = await stream.read_to_end()
            
            if message:
                decoded_message = message.decode()
                print(f"📩 Узел {self.node.peer_id()} получил сообщение от {peer_id}: {decoded_message}")
                self.received_messages.append((peer_id, decoded_message))
                
                # Отправляем подтверждение
                response = f"ACK: {decoded_message}"
                await stream.write(response.encode())
            
            # Закрываем поток после обработки
            await stream.close()
        
        except Exception as e:
            print(f"❌ Ошибка при обработке потока: {e}")
    
    async def connect_and_wait_auth(self, address, timeout=10):
        """
        Подключение к узлу с ожиданием завершения аутентификации
        
        :param address: Адрес узла для подключения
        :param timeout: Время ожидания в секундах
        :return: True в случае успеха, иначе False
        """
        # Подключаемся к узлу
        connected = await self.node.connect(address)
        
        if not connected:
            print(f"❌ Не удалось подключиться к {address}")
            return False
        
        # Ждем завершения аутентификации
        start_time = asyncio.get_event_loop().time()
        while asyncio.get_event_loop().time() - start_time < timeout:
            # Проверяем, есть ли адрес в списке аутентифицированных
            for peer_id in self.auth_completed:
                return True
            
            # Ждем немного времени перед следующей проверкой
            await asyncio.sleep(0.1)
        
        return False
    
    async def send_message(self, peer_id, message):
        """
        Отправка сообщения с получением ответа
        
        :param peer_id: ID узла-получателя
        :param message: Сообщение для отправки
        :return: Полученный ответ или None в случае ошибки
        """
        try:
            # Открываем поток к узлу
            stream = await self.node.open_stream(peer_id)
            print(f"📤 Узел {self.node.peer_id()} открыл поток к {peer_id}")
            
            # Отправляем сообщение
            print(f"📤 Отправка сообщения '{message}'")
            await stream.write(message.encode())
            await stream.write(message.encode())
            
            print("WRITE EOF!!!!!!!!!!!!!!!!")
            await stream.close()
            await asyncio.sleep(2)
            print("11 after sleep")
            
            # Читаем ответ из потока
            response_data = await stream.read_to_end()
            
            if response_data:
                response = response_data.decode()
                print(f"📩 Получен ответ: {response}")
                return response
            
            return None
        
        except Exception as e:
            print(f"❌  SEND MESSAGE Ошибка при отправке сообщения: {e}")
            raise
            return None
    
    def get_peer_id(self):
        """Получение ID узла"""
        return self.node.peer_id()
    
    def get_received_messages(self):
        """Получение списка полученных сообщений"""
        return self.received_messages
    
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
        print(f"🛑 Узел {self.node.peer_id()} остановлен")


@pytest.mark.asyncio
async def test_simple_message():
    """
    Тест отправки и получения сообщения между двумя узлами:
    1. Узел 1 (сервер) и Узел 2 (клиент) устанавливают соединение
    2. Узел 2 отправляет сообщение Узлу 1
    3. Узел 1 отвечает подтверждением
    4. Узел 2 получает подтверждение
    """
    # Создаём два тестовых узла
    server_node = TestSimpleNode(kad_server=True)
    client_node = TestSimpleNode()
    
    try:
        # Запускаем узлы на разных портах
        server_address = await server_node.start(port=9000)
        await client_node.start(port=9001)
        
        print(f"🚀 Сервер запущен на {server_address}")
        print(f"🚀 ID сервера: {server_node.get_peer_id()}")
        print(f"🚀 ID клиента: {client_node.get_peer_id()}")
        
        # Подключаем клиент к серверу
        print("🔌 Подключение клиента к серверу...")
        connected = await client_node.connect_and_wait_auth(server_address)
        assert connected, "Не удалось подключиться и аутентифицироваться"
        print("✅ Подключение установлено и аутентифицировано")
        
        # Создаем уникальное сообщение
        unique_id = str(uuid.uuid4())
        test_message = f"Test message #{unique_id}"
        
        # Клиент отправляет сообщение серверу
        print(f"📤 Клиент отправляет сообщение: {test_message}")
        response = await client_node.send_message(
            server_node.get_peer_id(), test_message
        )
        
        # Проверяем, что получен ответ
        assert response is not None, "Должен быть получен ответ"
        
        # Проверяем, что ответ содержит ожидаемый текст
        assert test_message in response, "Ответ должен содержать исходное сообщение"
        assert "ACK" in response, "Ответ должен содержать подтверждение"
        
        print("✅ Тест успешно пройден")
        print(f"📋 Отправленное сообщение: {test_message}")
        print(f"📋 Полученный ответ: {response}")
        
    finally:
        # Останавливаем узлы
        await server_node.stop()
        await client_node.stop()
        print("🛑 Тест завершен, узлы остановлены")


if __name__ == "__main__":
    # Для ручного запуска теста
    asyncio.run(test_simple_message())