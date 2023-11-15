# syntax=docker/dockerfile:1.4
# We only need the gramine dependencies on a production SGX server.
FROM switchboardlabs/gramine:dev AS builder

WORKDIR /home/root/switchboard
COPY ./Cargo.toml ./Cargo.toml
COPY ./Anchor.toml ./Anchor.toml
COPY ./programs/ ./programs/

WORKDIR /home/root/switchboard/switchboard-function
COPY ./switchboard-function/Cargo.toml ./Cargo.toml
COPY ./switchboard-function/Cargo.lock ./Cargo.lock
COPY ./switchboard-function/src/ ./src/

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/home/root/switchboard/switchboard-function/target \
    cargo build --release && \
    mv ./target/release/backfill-oracle-worker /backfill-oracle-worker

###############################################################
### Copy to final image
###############################################################
FROM switchboardlabs/gramine
WORKDIR /app

RUN mkdir -p /data/protected_files

###############################################################
### Linux Setup
###############################################################
ENV SGX_DCAP_VERSION="1.19.100.3-focal1"
RUN mv /etc/sgx_default_qcnl.conf /etc/sgx_default_qcnl.conf.bkup
RUN --mount=type=cache,id=apt-cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,id=apt-lib,target=/var/lib/apt,sharing=locked \
    --mount=type=cache,id=debconf,target=/var/cache/debconf,sharing=locked \
    set -exu && \
    DEBIAN_FRONTEND=noninteractive apt update && \
    apt -y --no-install-recommends install \
    libsgx-dcap-quote-verify-dev=${SGX_DCAP_VERSION} \
    libsgx-dcap-quote-verify=${SGX_DCAP_VERSION} \
    libsgx-dcap-default-qpl-dev=${SGX_DCAP_VERSION} \
    libsgx-dcap-default-qpl=${SGX_DCAP_VERSION} \
    libssl-dev
RUN rm -rf /etc/sgx_default_qcnl.conf && \
    mv /etc/sgx_default_qcnl.conf.bkup /etc/sgx_default_qcnl.conf

COPY ./configs/worker.manifest.template /app/worker.manifest.template
COPY ./configs/boot.sh /boot.sh
RUN chmod a+x /boot.sh

COPY --from=builder /backfill-oracle-worker /app/worker

RUN gramine-manifest /app/worker.manifest.template > /app/worker.manifest
RUN gramine-sgx-gen-private-key
RUN gramine-sgx-sign --manifest /app/worker.manifest --output /app/worker.manifest.sgx | tail -2 | tee /measurement.txt

ENTRYPOINT ["/bin/bash", "/boot.sh"]