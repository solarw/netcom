//! Тест механизма принятия решений для входящих потоков XStream

use super::*;
use std::time::Duration;
use std::task::{Context, Poll};
use tokio::time::timeout;
use tokio::sync::oneshot;
use libp2p::{PeerId, swarm::{ConnectionId, NetworkBehaviour}};
use crate::behaviour::XStreamNetworkBehaviour;
use crate::events::{IncomingConnectionApprovePolicy, StreamOpenDecisionSender, InboundUpgradeDecision};
use crate::handler::XStreamHandlerEvent;

/// Тестирует базовый механизм принятия решений для входящих потоков
#[tokio::test]
async fn test_inbound_decision_mechanism() {
    println!("🧪 Тестируем механизм принятия решений для входящих потоков XStream...");
    
    // Создаем два XStreamNetworkBehaviour с разными политиками
    let mut behaviour_auto = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::AutoApprove
    );
    
    let mut behaviour_manual = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    );
    
    println!("✅ Созданы два XStreamNetworkBehaviour:");
    println!("   - AutoApprove: автоматически одобряет входящие потоки");
    println!("   - ApproveViaEvent: генерирует события для ручного одобрения");
    
    // Проверяем, что политики установлены корректно
    assert_eq!(
        behaviour_auto.incoming_approve_policy,
        IncomingConnectionApprovePolicy::AutoApprove,
        "❌ Политика AutoApprove не установлена корректно"
    );
    
    assert_eq!(
        behaviour_manual.incoming_approve_policy,
        IncomingConnectionApprovePolicy::ApproveViaEvent,
        "❌ Политика ApproveViaEvent не установлена корректно"
    );
    
    println!("✅ Политики принятия решений установлены корректно");
    
    // Проверяем, что поведение создано без ошибок
    // PendingStreamsManager запускается в отдельной задаче и не должен быть доступен напрямую
    println!("✅ XStreamNetworkBehaviour созданы корректно");
    
    // Проверяем, что можем создавать новые stream_id
    let test_peer_id = PeerId::random();
    let stream_id_1 = behaviour_auto.request_open_stream(test_peer_id);
    let stream_id_2 = behaviour_auto.request_open_stream(test_peer_id);
    
    assert_ne!(stream_id_1, stream_id_2, "❌ Stream ID должны быть уникальными");
    println!("✅ Stream ID генерируются корректно и уникальны");
    
    println!("🎉 Тест механизма принятия решений пройден успешно!");
}

/// Тестирует обработку событий IncomingStreamRequest
#[tokio::test]
async fn test_incoming_stream_request_handling() {
    println!("🧪 Тестируем обработку событий IncomingStreamRequest...");
    
    let mut behaviour = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    );
    
    let test_peer_id = PeerId::random();
    let test_connection_id = ConnectionId::new_unchecked(1);
    
    // Создаем канал для принятия решения
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Создаем событие IncomingStreamRequest
    let event = XStreamHandlerEvent::IncomingStreamRequest {
        peer_id: test_peer_id,
        connection_id: test_connection_id,
        decision_sender: decision_sender.clone(),
    };
    
    // Обрабатываем событие в behaviour
    behaviour.on_connection_handler_event(test_peer_id, test_connection_id, event);
    
    // Проверяем, что событие было обработано и добавлено в очередь
    let mut events_processed = false;
    for _ in 0..10 {
        if let Poll::Ready(_) = behaviour.poll(&mut Context::from_waker(futures::task::noop_waker_ref())) {
            events_processed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    assert!(events_processed, "❌ Событие IncomingStreamRequest не было обработано");
    println!("✅ Событие IncomingStreamRequest обработано корректно");
    
    // Проверяем, что решение можно принять
    let decision_result = decision_sender.approve();
    assert!(decision_result.is_ok(), "❌ Не удалось отправить решение approve");
    println!("✅ Решение approve отправлено успешно");
    
    // Проверяем, что получатель получил решение
    match timeout(Duration::from_secs(1), response_receiver).await {
        Ok(Ok(decision)) => {
            assert_eq!(decision, InboundUpgradeDecision::Approved, "❌ Получено неверное решение");
            println!("✅ Получатель получил корректное решение Approved");
        }
        Ok(Err(_)) => panic!("❌ Канал решения был закрыт"),
        Err(_) => panic!("❌ Таймаут ожидания решения"),
    }
    
    println!("🎉 Тест обработки событий IncomingStreamRequest пройден успешно!");
}

/// Тестирует отклонение входящих потоков
#[tokio::test]
async fn test_inbound_stream_rejection() {
    println!("🧪 Тестируем отклонение входящих потоков...");
    
    let mut behaviour = XStreamNetworkBehaviour::new_with_policy(
        IncomingConnectionApprovePolicy::ApproveViaEvent
    );
    
    let test_peer_id = PeerId::random();
    let test_connection_id = ConnectionId::new_unchecked(1);
    
    // Создаем канал для принятия решения
    let (response_sender, response_receiver) = oneshot::channel();
    let decision_sender = StreamOpenDecisionSender::new(response_sender);
    
    // Создаем событие IncomingStreamRequest
    let event = XStreamHandlerEvent::IncomingStreamRequest {
        peer_id: test_peer_id,
        connection_id: test_connection_id,
        decision_sender: decision_sender.clone(),
    };
    
    // Обрабатываем событие в behaviour
    behaviour.on_connection_handler_event(test_peer_id, test_connection_id, event);
    
    // Проверяем, что событие было обработано
    let mut events_processed = false;
    for _ in 0..10 {
        if let Poll::Ready(_) = behaviour.poll(&mut Context::from_waker(futures::task::noop_waker_ref())) {
            events_processed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    assert!(events_processed, "❌ Событие IncomingStreamRequest не было обработано");
    
    // Отклоняем поток с причиной
    let rejection_reason = "Peer not authenticated".to_string();
    let decision_result = decision_sender.reject(rejection_reason.clone());
    assert!(decision_result.is_ok(), "❌ Не удалось отправить решение reject");
    println!("✅ Решение reject отправлено успешно");
    
    // Проверяем, что получатель получил решение с правильной причиной
    match timeout(Duration::from_secs(1), response_receiver).await {
        Ok(Ok(decision)) => {
            match decision {
                InboundUpgradeDecision::Rejected(reason) => {
                    assert_eq!(reason, rejection_reason, "❌ Получена неверная причина отклонения");
                    println!("✅ Получатель получил корректное решение Rejected с причиной: {}", reason);
                }
                _ => panic!("❌ Получено неверное решение, ожидалось Rejected"),
            }
        }
        Ok(Err(_)) => panic!("❌ Канал решения был закрыт"),
        Err(_) => panic!("❌ Таймаут ожидания решения"),
    }
    
    println!("🎉 Тест отклонения входящих потоков пройден успешно!");
}
