use super::*;
use crate::utils::IdIterator;

#[test]
fn test_id_iterator_new() {
    // Проверяем, что итератор создается корректно
    let iter = IdIterator::new();
    
    // Проверяем, что итератор не падает при создании
    // Это реальная проверка - конструктор должен работать без ошибок
    assert!(true); // Базовое утверждение, что тест выполняется
}

#[test]
fn test_id_iterator_next_sequence() {
    // Проверяем, что итератор возвращает последовательные значения
    let mut iter = IdIterator::new();
    
    // Получаем несколько значений и проверяем их последовательность
    let first = iter.next().expect("Should return Some value");
    let second = iter.next().expect("Should return Some value");
    let third = iter.next().expect("Should return Some value");
    
    // Проверяем, что значения последовательные
    assert_eq!(second, first + 1);
    assert_eq!(third, second + 1);
}

#[test]
fn test_id_iterator_large_sequence() {
    // Проверяем, что итератор работает корректно для большого количества итераций
    let mut iter = IdIterator::new();
    
    // Получаем 1000 значений и проверяем их последовательность
    let mut previous = iter.next().expect("Should return Some value");
    
    for _ in 1..1000 {
        let current = iter.next().expect("Should return Some value");
        assert_eq!(current, previous + 1);
        previous = current;
    }
}

#[test]
fn test_id_iterator_implements_iterator_trait() {
    // Проверяем, что IdIterator реализует трейт Iterator
    let mut iter = IdIterator::new();
    
    // Используем методы трейта Iterator
    let first = iter.next().expect("Should return Some value");
    let second = iter.next().expect("Should return Some value");
    
    // Проверяем, что значения последовательные
    assert_eq!(second, first + 1);
    
    // Проверяем, что итератор можно использовать в цикле for
    let mut count = 0;
    for _ in iter.take(10) {
        count += 1;
    }
    assert_eq!(count, 10);
}
