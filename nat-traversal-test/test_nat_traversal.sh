#!/bin/bash

echo "🧪 Тестирование NAT traversal между node1 и node2 через relay..."
echo "==============================================================="

# Загружаем фиксированные ключи из .env
if [ -f .env ]; then
    source .env
    echo "🔑 Используем фиксированные ключи из .env"
else
    echo "❌ .env файл не найден. Запустите generate_env для создания ключей."
    exit 1
fi

echo "🔑 Ключи загружены из .env"
echo "   - Relay: $RELAY_PEER_ID"
echo "   - Node1: $NODE1_PEER_ID"
echo "   - Node2: $NODE2_PEER_ID"

# Запускаем relay сервер в фоне
echo ""
echo "🚀 Запускаем relay сервер..."
NODE_KEY=$RELAY_KEY cargo run --bin relay &
RELAY_PID=$!

# Ждем запуска relay
echo "⏳ Ждем запуска relay сервера..."
sleep 5

echo "✅ Relay сервер запущен (PID: $RELAY_PID)"

# Запускаем node2 (пассивный узел) в фоне
echo ""
echo "🚀 Запускаем node2 (пассивный узел)..."
NODE_KEY=$NODE2_KEY cargo run --bin node -- --relay-address 127.0.0.1:15003 --relay-peer-id "$RELAY_PEER_ID" &
NODE2_PID=$!

# Ждем запуска node2
echo "⏳ Ждем запуска node2..."
sleep 5

echo "✅ Node2 запущен (PID: $NODE2_PID)"

# Запускаем node1 с подключением к node2 через relay
echo ""
echo "🔄 Запускаем node1 с подключением к node2 через relay..."
NODE_KEY=$NODE1_KEY timeout 30s cargo run --bin node -- --relay-address 127.0.0.1:15003 --relay-peer-id "$RELAY_PEER_ID" --target-peer "$NODE2_PEER_ID"
NODE1_EXIT=$?

echo ""
echo "📊 Результаты тестирования NAT traversal:"
echo "  Node1 завершился с кодом: $NODE1_EXIT"

# Завершаем процессы
echo ""
echo "🛑 Завершаем процессы..."
kill $NODE2_PID 2>/dev/null
wait $NODE2_PID 2>/dev/null
kill $RELAY_PID 2>/dev/null
wait $RELAY_PID 2>/dev/null

if [ $NODE1_EXIT -eq 0 ]; then
    echo "🎉 NAT traversal успешен! Node1 подключился к Node2 через relay!"
else
    echo "❌ Проблемы с NAT traversal между node1 и node2"
    exit 1
fi

echo ""
echo "✅ Тестирование NAT traversal завершено!"
