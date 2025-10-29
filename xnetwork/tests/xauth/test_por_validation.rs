// tests/xauth/test_por_validation.rs - Proof of Representation (PRIORITY 3)

use std::time::Duration;
use xnetwork::XRoutesConfig;

use crate::common::*;

#[tokio::test]
async fn test_por_validation() {
    let test_timeout = Duration::from_secs(15);
    
    println!("🧪 Testing Proof of Representation validation");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем что соединение установлено
        let connections = client_commander.get_all_connections().await.unwrap();
        println!("Found {} connections for PoR validation", connections.len());
        
        // TODO: Добавить проверку PoR валидации, когда API будет доступно
        // let por_status = client_commander.validate_por(server_peer_id).await;
        // assert!(por_status.is_valid(), "PoR should be valid");
        
        // Проверяем что соединение активно
        let active_connections = connections.iter().filter(|c| c.is_active()).count();
        println!("Active connections with PoR: {}", active_connections);
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ PoR validation test completed"),
        Err(_) => panic!("⏰ PoR validation test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_por_success() {
    let test_timeout = Duration::from_secs(15);
    
    println!("🧪 Testing successful PoR verification");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // TODO: Добавить проверку успешной верификации PoR
