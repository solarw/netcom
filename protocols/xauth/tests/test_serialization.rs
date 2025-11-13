// tests/test_serialization.rs - Тестирование сериализации ProofOfRepresentation

use libp2p::identity;
use xauth::por::por::{ProofOfRepresentation, PorUtils};
use std::time::Duration;

#[test]
fn test_por_serialization_roundtrip() {
    // Создаем тестовый POR
    let owner_keypair = PorUtils::generate_owner_keypair();
    let peer_id = PorUtils::peer_id_from_keypair(&owner_keypair);
    
    let por = ProofOfRepresentation::create(
        &owner_keypair,
        peer_id,
        Duration::from_secs(3600),
    ).expect("Failed to create POR");

    // Тестируем сериализацию через CBOR
    let serialized = serde_cbor::to_vec(&por).expect("Failed to serialize POR");
    println!("✅ Serialized POR to {} bytes", serialized.len());
    
    // Тестируем десериализацию
    let deserialized: ProofOfRepresentation = serde_cbor::from_slice(&serialized)
        .expect("Failed to deserialize POR");
    
    println!("✅ Successfully deserialized POR");
    
    // Проверяем, что данные совпадают
    assert_eq!(por.owner_public_key, deserialized.owner_public_key);
    assert_eq!(por.peer_id, deserialized.peer_id);
    assert_eq!(por.issued_at, deserialized.issued_at);
    assert_eq!(por.expires_at, deserialized.expires_at);
    assert_eq!(por.signature, deserialized.signature);
    
    println!("✅ All POR fields match after roundtrip");
}

#[test]
fn test_por_auth_request_serialization() {
    use xauth::definitions::{PorAuthRequest, AuthResult};
    use std::collections::HashMap;
    
    // Создаем тестовый POR
    let owner_keypair = PorUtils::generate_owner_keypair();
    let peer_id = PorUtils::peer_id_from_keypair(&owner_keypair);
    
    let por = ProofOfRepresentation::create(
        &owner_keypair,
        peer_id,
        Duration::from_secs(3600),
    ).expect("Failed to create POR");

    // Создаем запрос аутентификации
    let mut metadata = HashMap::new();
    metadata.insert("test_key".to_string(), "test_value".to_string());
    
    let auth_request = PorAuthRequest {
        por: por.clone(),
        metadata: metadata.clone(),
    };

    // Тестируем сериализацию запроса
    let serialized_request = serde_cbor::to_vec(&auth_request)
        .expect("Failed to serialize auth request");
    println!("✅ Serialized auth request to {} bytes", serialized_request.len());
    
    // Тестируем десериализацию запроса
    let deserialized_request: PorAuthRequest = serde_cbor::from_slice(&serialized_request)
        .expect("Failed to deserialize auth request");
    
    println!("✅ Successfully deserialized auth request");
    
    // Проверяем, что данные совпадают
    assert_eq!(auth_request.por.owner_public_key, deserialized_request.por.owner_public_key);
    assert_eq!(auth_request.por.peer_id, deserialized_request.por.peer_id);
    assert_eq!(auth_request.metadata, deserialized_request.metadata);
    
    println!("✅ All auth request fields match after roundtrip");
}

#[test]
fn test_por_auth_response_serialization() {
    use xauth::definitions::{PorAuthResponse, AuthResult};
    use std::collections::HashMap;
    
    // Тестируем успешный ответ
    let mut success_metadata = HashMap::new();
    success_metadata.insert("auth_result".to_string(), "success".to_string());
    
    let success_response = PorAuthResponse {
        result: AuthResult::Ok(success_metadata.clone()),
    };

    let serialized_success = serde_cbor::to_vec(&success_response)
        .expect("Failed to serialize success response");
    println!("✅ Serialized success response to {} bytes", serialized_success.len());
    
    let deserialized_success: PorAuthResponse = serde_cbor::from_slice(&serialized_success)
        .expect("Failed to deserialize success response");
    
    // Проверяем успешный ответ
    match (&success_response.result, &deserialized_success.result) {
        (AuthResult::Ok(orig_meta), AuthResult::Ok(deser_meta)) => {
            assert_eq!(orig_meta, deser_meta);
            println!("✅ Success response metadata matches");
        }
        _ => panic!("Unexpected response types"),
    }
    
    // Тестируем ответ с ошибкой
    let error_response = PorAuthResponse {
        result: AuthResult::Error("Authentication failed".to_string()),
    };

    let serialized_error = serde_cbor::to_vec(&error_response)
        .expect("Failed to serialize error response");
    println!("✅ Serialized error response to {} bytes", serialized_error.len());
    
    let deserialized_error: PorAuthResponse = serde_cbor::from_slice(&serialized_error)
        .expect("Failed to deserialize error response");
    
    // Проверяем ответ с ошибкой
    match (&error_response.result, &deserialized_error.result) {
        (AuthResult::Error(orig_err), AuthResult::Error(deser_err)) => {
            assert_eq!(orig_err, deser_err);
            println!("✅ Error response message matches");
        }
        _ => panic!("Unexpected response types"),
    }
}
