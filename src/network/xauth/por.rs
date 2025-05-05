use libp2p::{
    identity::Keypair,
    PeerId,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// Модуль для работы с Proof of Representation
pub mod por {
    use super::*;

    /// Структура Proof of Representation (POR)
    /// Представляет собой доверенность, удостоверяющую, что узел представляет владельца
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProofOfRepresentation {
        /// Открытый ключ владельца в формате байтов
        pub owner_public_key: Vec<u8>,
        
        /// PeerId узла, который представляет владельца
        pub peer_id: PeerId,
        
        /// Метка времени создания доверенности (UNIX timestamp в секундах)
        pub issued_at: u64,
        
        /// Срок действия доверенности (UNIX timestamp в секундах)
        pub expires_at: u64,
        
        /// Цифровая подпись, созданная закрытым ключом владельца
        pub signature: Vec<u8>,
    }
    
    impl ProofOfRepresentation {
        /// Создание нового POR с действительной подписью
        /// 
        /// # Аргументы
        /// * `owner_keypair` - Keypair владельца для подписи
        /// * `peer_id` - PeerId узла, которому делегируются полномочия
        /// * `validity_duration` - Срок действия доверенности
        /// 
        /// # Возвращает
        /// Новый экземпляр ProofOfRepresentation с действительной подписью
        pub fn create(
            owner_keypair: &Keypair,
            peer_id: PeerId,
            validity_duration: Duration,
        ) -> Result<Self, String> {
            // Получаем текущее время
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Ошибка системного времени: {}", e))?
                .as_secs();
            
            // Вычисляем время истечения срока действия
            let expires_at = now + validity_duration.as_secs();
            
            // Получаем байты публичного ключа (используем непосредственно to_protobuf_encoding на keypair)
            let public_key_bytes = owner_keypair.to_protobuf_encoding()
                .map_err(|e| format!("Ошибка кодирования ключа: {}", e))?;
            
            // Подготавливаем данные для подписи
            let message = Self::prepare_message_for_signing(
                &public_key_bytes,
                &peer_id,
                now,
                expires_at,
            );
            
            // Создаем подпись
            let signature = owner_keypair.sign(&message)
                .map_err(|e| format!("Ошибка создания подписи: {}", e))?;
            
            Ok(Self {
                owner_public_key: public_key_bytes,
                peer_id,
                issued_at: now,
                expires_at,
                signature,
            })
        }
        
        /// Создание POR с заданным временем начала и окончания (для тестирования)
        pub fn create_with_times(
            owner_keypair: &Keypair,
            peer_id: PeerId,
            issued_at: u64,
            expires_at: u64,
        ) -> Result<Self, String> {
            // Получаем байты публичного ключа (используем непосредственно to_protobuf_encoding на keypair)
            let public_key_bytes = owner_keypair.to_protobuf_encoding()
                .map_err(|e| format!("Ошибка кодирования ключа: {}", e))?;
            
            // Подготавливаем данные для подписи
            let message = Self::prepare_message_for_signing(
                &public_key_bytes,
                &peer_id,
                issued_at,
                expires_at,
            );
            
            // Создаем подпись
            let signature = owner_keypair.sign(&message)
                .map_err(|e| format!("Ошибка создания подписи: {}", e))?;
            
            Ok(Self {
                owner_public_key: public_key_bytes,
                peer_id,
                issued_at,
                expires_at,
                signature,
            })
        }
        
        /// Проверка действительности POR
        ///
        /// # Возвращает
        /// `Ok(())` если POR действителен, иначе `Err` с описанием проблемы
        pub fn validate(&self) -> Result<(), String> {
            // Проверка срока действия
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Ошибка системного времени: {}", e))?
                .as_secs();
            
            if now < self.issued_at {
                return Err("POR еще не вступил в силу".to_string());
            }
            
            if now > self.expires_at {
                return Err("Срок действия POR истек".to_string());
            }
            
            // Проверка подписи
            self.verify_signature()
        }
        
        /// Проверка подписи POR
        ///
        /// # Возвращает
        /// `Ok(())` если подпись верна, иначе `Err` с описанием проблемы
        fn verify_signature(&self) -> Result<(), String> {
            // Преобразуем байты в ключ
            let keypair = Keypair::from_protobuf_encoding(&self.owner_public_key)
                .map_err(|e| format!("Неверный формат ключа: {}", e))?;
            
            // Получаем публичный ключ
            let public_key = keypair.public();
            
            // Подготавливаем данные для проверки подписи
            let message = Self::prepare_message_for_signing(
                &self.owner_public_key,
                &self.peer_id,
                self.issued_at,
                self.expires_at,
            );
            
            // Проверяем подпись
            if public_key.verify(&message, &self.signature) {
                Ok(())
            } else {
                Err("Неверная подпись".to_string())
            }
        }
        
        /// Подготовка сообщения для подписи/проверки
        fn prepare_message_for_signing(
            owner_public_key: &[u8],
            peer_id: &PeerId,
            issued_at: u64,
            expires_at: u64,
        ) -> Vec<u8> {
            // Конкатенируем все данные, которые должны быть подписаны
            let mut message = Vec::new();
            
            // Добавляем открытый ключ владельца
            message.extend_from_slice(owner_public_key);
            
            // Добавляем peer_id как строку
            let peer_id_str = peer_id.to_string();
            message.extend_from_slice(peer_id_str.as_bytes());
            
            // Добавляем issued_at и expires_at как байты
            message.extend_from_slice(&issued_at.to_le_bytes());
            message.extend_from_slice(&expires_at.to_le_bytes());
            
            message
        }
        
        /// Проверка, истёк ли срок действия POR
        pub fn is_expired(&self) -> Result<bool, String> {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Ошибка системного времени: {}", e))?
                .as_secs();
            
            Ok(now > self.expires_at)
        }
        
        /// Получение оставшегося времени действия в секундах
        pub fn remaining_time(&self) -> Result<Option<u64>, String> {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("Ошибка системного времени: {}", e))?
                .as_secs();
            
            if now > self.expires_at {
                Ok(None)
            } else {
                Ok(Some(self.expires_at - now))
            }
        }
    }
    
    /// Вспомогательные функции для работы с ключами
    pub struct PorUtils;
    
    impl PorUtils {
        /// Создание новой пары ключей для владельца
        pub fn generate_owner_keypair() -> Keypair {
            Keypair::generate_ed25519()
        }
        
        /// Создание ключевой пары из существующего закрытого ключа в формате protobuf
        pub fn keypair_from_bytes(secret_key_bytes: &[u8]) -> Result<Keypair, String> {
            Keypair::from_protobuf_encoding(secret_key_bytes)
                .map_err(|e| format!("Неверный формат ключа: {}", e))
        }
        
        /// Получение PeerId из ключевой пары
        pub fn peer_id_from_keypair(keypair: &Keypair) -> PeerId {
            keypair.public().to_peer_id()
        }
    }
}


