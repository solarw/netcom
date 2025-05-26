// tests/test_diagnostic.rs

use std::time::Duration;
use xnetwork::XRoutesConfig;

mod common;
use common::*;

#[tokio::test]
async fn test_node_lifecycle() {
    println!("🧪 Testing node lifecycle...");
    
    // Создаем узел
    let (mut node, _commander, _events, peer_id) = 
        create_test_node_with_config(XRoutesConfig::client()).await
        .expect("Failed to create test node");
    
    println!("✅ Node created: {}", peer_id);
    
    // Запускаем узел с коротким временем жизни
    let handle = tokio::spawn(async move {
        println!("🚀 Node starting...");
        // Используем быструю очистку
        tokio::select! {
            _ = node.run_with_cleanup_interval(Duration::from_millis(100)) => {
                println!("📴 Node run completed");
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                println!("⏰ Node run timed out (expected)");
            }
        }
    });
    
    // Ждем немного
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("⏹️ Aborting node...");
    
    // Принудительно останавливаем
    handle.abort();
    
    // Ждем завершения
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    println!("✅ Node lifecycle test completed");
}

#[tokio::test] 
async fn test_bootstrap_creation_only() {
    println!("🧪 Testing bootstrap server creation only...");
    
    // Создаем bootstrap сервер
    let (handle, addr, peer_id) = create_bootstrap_server().await
        .expect("Failed to create bootstrap server");
    
    println!("✅ Bootstrap created: {} at {}", peer_id, addr);
    
    // Сразу останавливаем
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.abort();
    
    tokio::time::sleep(Duration::from_millis(200)).await;
    println!("✅ Bootstrap creation test completed");
}