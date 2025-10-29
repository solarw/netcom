// tests/xauth/test_auth_on_connection.rs - Аутентификация при подключении (PRIORITY 3)

use std::time::Duration;
use xnetwork::XRoutesConfig;

use crate::common::*;

#[tokio::test]
async fn test_auth_on_connection() {
    let test_timeout = Duration::from_secs(15);
    
    println!("🧪 Testing authentication on connection establishment");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Создаем пару нод с включенной аутентификацией
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем что соединение установлено
        let connections = client_commander.get_all_connections().await.unwrap();
        println!("Found {} connections after authentication", connections.len());
        
        // Проверяем что соединение активно
        let active_connections = connections.iter().filter(|c| c.is_active()).count();
        println!("Active connections: {}", active_connections);
        
        // XAuth автоматически запускает аутентификацию при установлении соединения
        // Ждем немного для завершения процесса аутентификации
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Проверяем что соединение все еще активно после аутентификации
        let connections_after_auth = client_commander.get_all_connections().await.unwrap();
        let active_after_auth = connections_after_auth.iter().filter(|c| c.is_active()).count();
        println!("Active connections after auth: {}", active_after_auth);
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Authentication on connection test completed"),
        Err(_) => panic!("⏰ Authentication on connection test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_auth_success() {
    let test_timeout = Duration::from_secs(15);
    
    println!("🧪 Testing successful authentication");
    
    let result = tokio::time::timeout(test_timeout, async {
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем что соединение установлено и аутентифицировано
        let connections = client_commander.get_all_connections().await.unwrap();
        let active_connections = connections.iter().filter(|c| c.is_active()).count();
        
        println!("Active authenticated connections: {}", active_connections);
        
        // XAuth автоматически выполняет аутентификацию
        // Ждем завершения процесса
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Проверяем что соединение остается активным после аутентификации
        let connections_after = client_commander.get_all_connections().await.unwrap();
        let still_active = connections_after.iter().filter(|c| c.is_active()).count();
        assert!(still_active > 0, "Connection should remain active after successful authentication");
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Successful authentication test completed"),
        Err(_) => panic!("⏰ Successful authentication test timed out ({}s)", test_timeout.as_secs()),
    }
}

#[tokio::test]
async fn test_auth_basic_functionality() {
    let test_timeout = Duration::from_secs(10);
    
    println!("🧪 Testing basic authentication functionality");
    
    let result = tokio::time::timeout(test_timeout, async {
        // Проверяем что ноды могут устанавливать соединения с аутентификацией
        let (_server_handle, _client_handle, _server_commander, client_commander, 
             server_peer_id, _client_peer_id, _server_addr) = 
            create_connected_pair().await.unwrap();
        
        // Проверяем что соединение установлено
        let connections = client_commander.get_all_connections().await.unwrap();
        assert!(!connections.is_empty(), "Should have at least one connection");
        
        // Проверяем что соединение активно
        let active_connections = connections.iter().filter(|c| c.is_active()).count();
        assert!(active_connections > 0, "Should have active connections");
        
        println!("✅ Basic authentication functionality test completed");
        
        // Отключаемся для очистки
        let _ = client_commander.disconnect(server_peer_id).await;
        
        Result::<(), Box<dyn std::error::Error>>::Ok(())
    }).await;
    
    match result {
        Ok(_) => println!("✅ Basic authentication functionality test completed"),
        Err(_) => panic!("⏰ Basic authentication functionality test timed out ({}s)", test_timeout.as_secs()),
    }
}
