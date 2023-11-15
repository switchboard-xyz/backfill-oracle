#!/bin/bash

if [[ "${UID}" -ne 0 ]]; then
    echo "Please run this script with root privileges."
fi

(
  /restart_aesm.sh
)

echo "Starting enclave.."
gramine-sgx /app/worker
