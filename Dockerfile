FROM python:3.14-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /pbsm

COPY Cargo.toml Cargo.lock ./
COPY crates/pbsm-core/Cargo.toml crates/pbsm-core/Cargo.toml
COPY crates/pbsm-python/Cargo.toml crates/pbsm-python/Cargo.toml
COPY crates/pbsm-python/pyproject.toml crates/pbsm-python/pyproject.toml

RUN mkdir -p crates/pbsm-core/src && echo "" > crates/pbsm-core/src/lib.rs
RUN mkdir -p crates/pbsm-python/src && echo "" > crates/pbsm-python/src/lib.rs

ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
RUN cargo build --release -p pbsm-python 2>/dev/null || true

COPY crates/pbsm-core/src/ crates/pbsm-core/src/
COPY crates/pbsm-python/src/ crates/pbsm-python/src/

RUN touch crates/pbsm-core/src/lib.rs crates/pbsm-python/src/lib.rs
RUN cargo build --release -p pbsm-python

RUN pip install --no-cache-dir maturin
RUN cd crates/pbsm-python && \
    PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin build --release

FROM python:3.14-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /pbsm

COPY --from=builder /pbsm/target/wheels/*.whl /tmp/
RUN pip install --no-cache-dir /tmp/*.whl

COPY adapters/tool_adapter/ /pbsm/adapters/tool_adapter/
RUN pip install --no-cache-dir /pbsm/adapters/tool_adapter/

ENV PYTHONPATH=/pbsm
ENV PBSM_DATA_DIR=/pbsm/data

RUN mkdir -p /pbsm/data

EXPOSE 8080

CMD ["python", "-m", "pbsm_tool_adapter", "--serve"]
