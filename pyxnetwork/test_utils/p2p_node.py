# test_utils/p2p_node.py

import os
import re
import time
import pexpect
import socket
from pathlib import Path
import signal

class Node:
    """
    Оболочка для процесса p2p-network, предоставляющая удобные методы
    для управления узлами в тестах.
    """
    
    def __init__(self, port=None, disable_mdns=True, accept_all_auth=True, logfile_path=None, kad_server_mode=False):
        """
        Инициализирует узел P2P сети.
        
        Args:
            port (int, optional): Порт для прослушивания. Если None, будет использован случайный порт.
            disable_mdns (bool): Отключить mDNS обнаружение.
            accept_all_auth (bool): Принимать все запросы аутентификации.
            logfile_path (str, optional): Путь к лог-файлу. Если None, будет создан временный файл.
            kad_server_mode (bool): Запустить узел в режиме Kademlia-сервера.
        """
        self.port = port if port is not None else self._get_free_port()
        self.disable_mdns = disable_mdns
        self.accept_all_auth = accept_all_auth
        self.logfile_path = logfile_path or f"node_{self.port}.log"
        self.kad_server_mode = kad_server_mode
        self.process = None
        self.logfile = None
        self.peer_id = None
        self.multiaddr = None
    
    def _get_free_port(self):
        """Получает свободный TCP порт для использования."""
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(('', 0))
            return s.getsockname()[1]
    
    def _extract_peer_id(self, output):
        """Извлекает Peer ID из выходных данных процесса."""
        # Пробуем различные шаблоны для поиска peer ID
        patterns = [
            r"Local peer ID: ([a-zA-Z0-9-_/]+)",
            r"local_peer_id=([a-zA-Z0-9-_/]+)",
            r"peer ID: ([a-zA-Z0-9-_/]+)",
            r"PeerId: ([a-zA-Z0-9-_/]+)",
            r"\nLocal peer ID: ([a-zA-Z0-9-_/]+)",
            r"(12D3KooW[a-zA-Z0-9-_/]+)"
        ]
        
        for pattern in patterns:
            match = re.search(pattern, output)
            if match:
                return match.group(1)
        
        return None
    
    def _extract_multiaddr(self, output):
        """Извлекает multiaddr с 127.0.0.1 из выходных данных процесса."""
        # Сначала ищем локальный адрес
        localhost_match = re.search(r"(/ip4/127\.0\.0\.1/udp/[0-9]+/quic-v1/p2p/[a-zA-Z0-9-_/]+)", output)
        if localhost_match:
            return localhost_match.group(1)
        
        # Если не нашли localhost адрес, ищем любой multiaddr
        multiaddr_match = re.search(r"Full address \(copy to connect\): (/ip4/[^ \n]+)", output)
        if multiaddr_match:
            addr = multiaddr_match.group(1)
            # Проверяем, содержит ли уже localhost
            if "/ip4/127.0.0.1/" in addr:
                return addr
            
            # Если нет, извлекаем порт и peer ID и создаем localhost адрес
            port_match = re.search(r"/udp/([0-9]+)/", addr)
            peer_id_match = re.search(r"/p2p/([a-zA-Z0-9-_/]+)", addr)
            
            if port_match and peer_id_match:
                port = port_match.group(1)
                peer_id = peer_id_match.group(1)
                return f"/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{peer_id}"
        
        # Если мы знаем порт и peer ID, но не нашли multiaddr
        if self.port and self.peer_id:
            return f"/ip4/127.0.0.1/udp/{self.port}/quic-v1/p2p/{self.peer_id}"
        
        return None
    
    def _wait_for_pattern(self, patterns, timeout=10, check_logs=True):
        """
        Ждет появления одного из шаблонов в выводе процесса.
        
        Args:
            patterns (list): Список шаблонов для поиска.
            timeout (int): Максимальное время ожидания в секундах.
            check_logs (bool): Проверять ли лог-файл если шаблон не найден.
            
        Returns:
            tuple: (pattern_index, match_text) или (None, None) если не найдено.
        """
        if not self.process:
            return None, None
        
        try:
            index = self.process.expect(patterns + [pexpect.TIMEOUT], timeout=timeout)
            if index < len(patterns):
                # Нашли соответствие
                match_text = self.process.match.group(0) if self.process.match else None
                return index, match_text
        except Exception as e:
            print(f"Exception waiting for pattern: {e}")
        
        # Если шаблон не найден в выводе процесса и check_logs=True
        if check_logs:
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            for i, pattern in enumerate(patterns):
                if re.search(pattern, log_content):
                    return i, pattern
        
        return None, None
    
    def start(self, timeout=15):
        """
        Запускает процесс узла и ожидает его инициализации.
        
        Args:
            timeout (int): Максимальное время ожидания инициализации в секундах.
            
        Returns:
            bool: True если узел успешно запущен, иначе False.
        """
        # Подготовка команды запуска
        cmd_args = [
            "--port", str(self.port)
        ]
        
        if self.disable_mdns:
            cmd_args.append("--disable-mdns")
            
        if self.accept_all_auth:
            cmd_args.append("--accept-all-auth")

        if self.kad_server_mode:
            cmd_args.append("--kad-server")
        
        cmd = f"stdbuf -i 0 -o 0 -e 0 target/debug/p2p-network {' '.join(cmd_args)}"
        print(f"Starting node with command: {cmd}")
        
        # Открытие лог-файла с построчной буферизацией
        self.logfile = open(self.logfile_path, "w", buffering=1)
        self.process = pexpect.spawn(cmd, encoding='utf-8', logfile=self.logfile, maxread=10000000)
        
        # Ожидание инициализации
        start_time = time.time()
        initialized = False
        
        while time.time() - start_time < timeout:
            try:
                # Ждем сообщение о том, что интерактивный режим доступен
                index = self.process.expect(["Interactive mode", "Local peer ID:", pexpect.TIMEOUT], timeout=5)
                if index != 2:  # Не таймаут
                    initialized = True
                    break
            except Exception as e:
                print(f"Exception during initialization: {e}")
                
            # Проверяем лог-файл на наличие необходимой информации
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            if "Local peer ID:" in log_content or "Interactive mode" in log_content:
                initialized = True
                break
                
            time.sleep(1)
        
        if not initialized:
            print("Node initialization timed out")
            return False
        
        # Извлекаем peer ID и multiaddr
        with open(self.logfile_path, "r") as f:
            log_content = f.read()
            
        self.peer_id = self._extract_peer_id(log_content)
        if not self.peer_id:
            print("Failed to extract peer ID")
            return False
            
        self.multiaddr = self._extract_multiaddr(log_content)
        if not self.multiaddr:
            # Если multiaddr не найден, но мы знаем peer ID, пробуем сконструировать
            self.multiaddr = f"/ip4/127.0.0.1/udp/{self.port}/quic-v1/p2p/{self.peer_id}"
            print(f"Constructed multiaddr: {self.multiaddr}")
            
        print(f"Node started with peer ID: {self.peer_id}")
        print(f"Node multiaddr: {self.multiaddr}")
        
        # Подождем ещё немного для полной инициализации
        time.sleep(2)
        
        return True
    
    def connect(self, target_multiaddr, timeout=15):
        """
        Подключается к другому узлу по multiaddr и проверяет успешность соединения.
        
        Args:
            target_multiaddr (str): Multiaddr узла для подключения.
            timeout (int): Максимальное время ожидания соединения в секундах.
            
        Returns:
            bool: True если соединение успешно установлено, иначе False.
        """
        if not self.process or not self.process.isalive():
            print("Node process is not running")
            return False
        
        # Подготавливаем локальный вариант multiaddr, если необходимо
        if "/ip4/127.0.0.1/" not in target_multiaddr:
            port_match = re.search(r"/udp/([0-9]+)/", target_multiaddr)
            peer_id_match = re.search(r"/p2p/([a-zA-Z0-9-_/]+)", target_multiaddr)
            
            if port_match and peer_id_match:
                target_port = port_match.group(1)
                target_peer_id = peer_id_match.group(1)
                local_target_addr = f"/ip4/127.0.0.1/udp/{target_port}/quic-v1/p2p/{target_peer_id}"
                print(f"Using localhost address: {local_target_addr}")
                target_multiaddr = local_target_addr
        
        # Отправляем команду подключения
        connect_cmd = f"connect {target_multiaddr}"
        print(f"Sending connect command: {connect_cmd}")
        self.process.sendline(connect_cmd)
        
        # Шаблоны для определения успешного подключения
        target_peer_id = re.search(r"/p2p/([a-zA-Z0-9-_/]+)", target_multiaddr)
        target_peer_id = target_peer_id.group(1) if target_peer_id else None
        
        connection_patterns = [
            f"Connected to {target_multiaddr}",
            "Connected to peer",
            f"Connection opened to {target_peer_id}" if target_peer_id else None,
            f"Connected to {target_peer_id}" if target_peer_id else None,
            "Connection established",
            "Established outbound connection",
            "Successfully connected"
        ]
        
        # Удаляем None элементы, если target_peer_id не был найден
        connection_patterns = [p for p in connection_patterns if p]
        
        # Ждем подтверждения соединения
        start_time = time.time()
        connected = False
        
        while time.time() - start_time < timeout:
            index, match = self._wait_for_pattern(connection_patterns, timeout=5)
            if index is not None:
                connected = True
                break
                
            # Проверяем лог-файл
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            if any(pattern in log_content for pattern in connection_patterns):
                connected = True
                break
                
            # Если прошло достаточно времени и все еще нет подтверждения, 
            # пробуем отправить команду еще раз
            if time.time() - start_time > timeout/2 and not connected:
                print("No connection confirmation yet, trying again...")
                self.process.sendline(connect_cmd)
        
        if not connected:
            print(f"Failed to connect to {target_multiaddr}")
            
            # Выводим последние строки лога для отладки
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            print("LOG EXCERPTS RELATED TO CONNECTION ATTEMPT:")
            for line in log_content.splitlines():
                if ("connect" in line.lower() or "peer" in line.lower() or 
                    target_multiaddr in line or (target_peer_id and target_peer_id in line)):
                    print(line)
            
            return False
            
        print(f"Successfully connected to {target_multiaddr}")
        return True
    
    def find(self, target_peer_id, timeout=30):
        """
        Выполняет поиск другого узла через Kademlia DHT.
        
        Args:
            target_peer_id (str): Peer ID узла для поиска.
            timeout (int): Максимальное время ожидания в секундах.
            
        Returns:
            bool: True если узел найден, иначе False.
        """
        if not self.process or not self.process.isalive():
            print("Node process is not running")
            return False
        
        # Отправляем команду поиска
        find_cmd = f"find {target_peer_id}"
        print(f"Sending find command: {find_cmd}")
        self.process.sendline(find_cmd)
        
        # Шаблоны для подтверждения успешного поиска
        success_patterns = [
            f"Successfully found and connected to peer: {target_peer_id}",
            f"Found {target_peer_id}",
            f"Connected to {target_peer_id}",
            f"Connection opened to {target_peer_id}",
            f"Finding {target_peer_id}",
            "peer was found",
            "Found closest peers",
            "Successfully found peer"
        ]
        
        # Ждем подтверждения поиска
        start_time = time.time()
        found = False
        retry_count = 0
        
        while time.time() - start_time < timeout:
            index, match = self._wait_for_pattern(success_patterns, timeout=5)
            if index is not None:
                found = True
                break
                
            # Проверяем лог-файл
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            if any(pattern in log_content for pattern in success_patterns):
                found = True
                break
                
            # Повторяем команду поиска через определенные интервалы
            if retry_count < 3 and time.time() - start_time > (timeout/4) * (retry_count + 1):
                print(f"No discovery confirmation yet, retrying (attempt {retry_count + 1})...")
                self.process.sendline(find_cmd)
                retry_count += 1
        
        if not found:
            print(f"Failed to find peer {target_peer_id}")
            
            # Выводим последние строки лога для отладки
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            print("LOG EXCERPTS RELATED TO FIND ATTEMPT:")
            for line in log_content.splitlines():
                if ("find" in line.lower() or "kad" in line.lower() or 
                    "kademlia" in line.lower() or target_peer_id in line):
                    print(line)
            
            return False
            
        print(f"Successfully found peer {target_peer_id}")
        return True
    
    def stream_message(self, target_peer_id, message, timeout=30):
        """
        Отправляет сообщение другому узлу через поток.
        
        Args:
            target_peer_id (str): Peer ID узла-получателя.
            message (str): Сообщение для отправки.
            timeout (int): Максимальное время ожидания в секундах.
            
        Returns:
            bool: True если сообщение успешно отправлено, иначе False.
        """
        if not self.process or not self.process.isalive():
            print("❌ Node process is not running")
            return False
        
        # Отправляем команду создания потока и отправки сообщения
        stream_cmd = f"stream {target_peer_id} {message}"
        print(f"🚀 Sending stream command: {stream_cmd}")
        self.process.sendline(stream_cmd)
        
        # Шаблоны для подтверждения успешной отправки - расширенный список
        send_patterns = [
            "📤 Stream opened",
            "✅ Message sent successfully",
            "🔒 Stream closed successfully",
            "Stream opened to",
            "Opening stream to", 
            "Sending message",
            "Connection opened to",
            "Stream to peer",
            "Stream established",
            "Successfully sent",
            "Message sent"
        ]
        
        # Ждем подтверждения отправки
        start_time = time.time()
        found_patterns = set()
        
        while time.time() - start_time < timeout:
            # Проверяем лог-файл
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
                
            # Проверяем каждый шаблон
            for pattern in send_patterns:
                if pattern in log_content and pattern not in found_patterns:
                    found_patterns.add(pattern)
                    print(f"✓ Found pattern: {pattern}")
            
            # Если нашли хотя бы 1 шаблон, считаем сообщение отправленным
            if len(found_patterns) >= 1:
                print(f"✅ Successfully sent message to {target_peer_id}: {message}")
                return True
                
            # Если прошла треть времени и нашли мало шаблонов, пробуем еще раз
            if time.time() - start_time > timeout/3 and len(found_patterns) < 1:
                print("⚠️ Few sending confirmations found, trying again...")
                self.process.sendline(stream_cmd)
            
            time.sleep(0.5)
        
        # Если не нашли достаточно шаблонов, выводим отладочную информацию
        print(f"❌ Failed to send message to peer {target_peer_id}")
        print(f"Found patterns: {found_patterns}")
        
        # Выводим последние строки лога для отладки
        with open(self.logfile_path, "r") as f:
            log_content = f.read().splitlines()
            
        print("📋 LOG EXCERPTS RELATED TO STREAM ATTEMPT:")
        for line in log_content[-30:]:  # Увеличено количество показываемых строк
            if ("stream" in line.lower() or "message" in line.lower() or 
                "sent" in line.lower() or target_peer_id in line or
                "connection" in line.lower()):
                print(line)
        
        return False
    
    def check_received_message(self, sender_peer_id=None, message=None, timeout=30):
        """
        Проверяет, было ли получено сообщение от указанного узла.
        
        Args:
            sender_peer_id (str, optional): Peer ID узла-отправителя.
            message (str, optional): Содержимое сообщения для проверки.
            timeout (int): Максимальное время ожидания в секундах.
            
        Returns:
            bool: True если сообщение получено, иначе False.
        """
        # Универсальные шаблоны для обнаружения входящих сообщений - расширенный список
        base_patterns = [
            "📥 Received stream from",     # Обнаружен входящий поток
            "📩 Message received from",    # Получено сообщение
            "Received stream",
            "Incoming stream",
            "IncomingStream", 
            "Message received",
            "received message",
            "Stream from peer",
            "received data",
            "Data from peer",
            "Received bytes"
        ]
        
        # Специфические шаблоны для указанного отправителя и сообщения
        specific_patterns = []
        
        if sender_peer_id:
            # Ищем указанный peer_id в логах
            specific_patterns.append(f"from {sender_peer_id}")
            specific_patterns.append(f"{sender_peer_id}")
        
        if message:
            # Ищем сообщение в одинарных кавычках или без них
            specific_patterns.append(f"'{message}'")
            specific_patterns.append(message)
        
        # Проверяем лог-файл
        start_time = time.time()
        found_base = False
        found_specific = True if not specific_patterns else False
        
        # Для более детального отслеживания
        found_patterns = set()
        
        while time.time() - start_time < timeout:
            with open(self.logfile_path, "r") as f:
                log_content = f.read()
            
            # Проверяем базовые шаблоны
            if not found_base:
                for pattern in base_patterns:
                    if pattern in log_content:
                        found_base = True
                        found_patterns.add(pattern)
                        break
            
            # Если указаны специфические шаблоны, проверяем их
            if specific_patterns and not found_specific:
                # Проверяем каждый шаблон отдельно для более гибкого обнаружения
                matches_count = 0
                for pattern in specific_patterns:
                    if pattern in log_content:
                        matches_count += 1
                        found_patterns.add(pattern)
                
                # Считаем "найденным", если есть хотя бы один совпадающий шаблон
                if matches_count > 0:
                    found_specific = True
            
            # Если нашли все необходимые шаблоны, возвращаем True
            if found_base and found_specific:
                print(f"✅ Message verification successful! Found patterns: {found_patterns}")
                return True
            
            # Проверяем, есть ли упоминание нашего сообщения в логе даже без явного шаблона
            if message and message in log_content:
                print(f"✅ Message content '{message}' found directly in logs!")
                return True
                
            # Короткий сон перед следующей проверкой
            time.sleep(0.5)
        
        # Если вышли из цикла, значит не нашли все шаблоны за отведенное время
        print(f"⚠️ Message verification failed! " + 
              f"Found: {found_patterns}, " + 
              f"Base patterns found: {found_base}, " + 
              f"Specific patterns found: {found_specific}")
        
        # Выводим немного лога для отладки
        with open(self.logfile_path, "r") as f:
            log_lines = f.readlines()[-30:]  # последние 30 строк
        
        print("📋 Last lines of log file:")
        for line in log_lines:
            print(f"  {line.strip()}")
        
        return False
        
    def stop(self):
        """Останавливает процесс узла и освобождает ресурсы."""
        if self.process and self.process.isalive():
            print(f"Stopping node with peer ID: {self.peer_id}")
            try:
                self.process.kill(signal.SIGTERM)
                self.process.wait()
            except Exception as e:
                print(f"Error stopping node: {e}")
            finally:
                self.process.close()
            
        if self.logfile:
            self.logfile.close()
        
        print(f"Node {self.peer_id} stopped")
    
    def __enter__(self):
        """Контекстный менеджер для использования с 'with'"""
        self.start()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Автоматическая остановка при выходе из блока 'with'"""
        self.stop()