FROM rust:1.80-slim

WORKDIR /app

# Устанавливаем зависимости для сборки
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Копируем файлы проекта
COPY Cargo.toml .
COPY src/ ./src/

# Собираем приложение
RUN cargo build --release

# Создаем точку входа
CMD ["/app/target/release/relay"]
