#!/bin/bash

ANCHOR_PROVIDER_URL=" https://api.devnet.solana.com"

kill_child_processes() {
    pkill -P $$
}

function create_order() {
    local num=$1
    if [[ -z "${num}" ]]; then
        echo -e "NUMBER NOT PROVIDED"
        return 1
    fi

    keypair="${HOME}/keypairs/devnet/wallet${num}.json"

    if [[ ! -f "${keypair}" ]]; then
        solana-keygen new  --no-bip39-passphrase --silent -o "${keypair}"
        solana transfer --allow-unfunded-recipient --from "${HOME}/.config/solana/id.json" --url "${ANCHOR_PROVIDER_URL}" $(solana-keygen pubkey "${keypair}") 1
    fi

    while true; do
        ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL}" ANCHOR_WALLET="${keypair}" ./node_modules/.bin/tsx scripts/create_order.ts "${keypair}"

        # Generate a random number between 1 and 8
        jitter=$((1 + RANDOM % 10))
        sleep "${jitter}"
    done
}
trap kill_child_processes EXIT INT TERM

# trap 'echo "A create_order thread crashed"; exit' CHLD

for i in {1..100}; do
    create_order "${i}" &
done

wait

