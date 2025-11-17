FROM rust:1.90-slim as builder

# Устанавливаем зависимости для сборки
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Копируем только необходимые файлы для сборки
WORKDIR /app
COPY protocols/ protocols/
COPY xnetwork2/ xnetwork2/
COPY command-swarm/ command-swarm/
COPY nat-traversal-bin/Cargo.toml nat-traversal-bin/Cargo.toml
COPY nat-traversal-bin/src/ nat-traversal-bin/src/

# Собираем бинарники
RUN cd nat-traversal-bin && cargo build --release --bin relay && \
    cargo build --release --bin node && \
    cargo build --release --bin generate_env

FROM debian:bookworm-slim

# Устанавливаем необходимые зависимости
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Копируем готовые бинарники
COPY --from=builder /app/nat-traversal-bin/target/release/relay /usr/local/bin/
COPY --from=builder /app/nat-traversal-bin/target/release/node /usr/local/bin/
COPY --from=builder /app/nat-traversal-bin/target/release/generate_env /usr/local/bin/

# Указываем команду по умолчанию
CMD ["relay"]
