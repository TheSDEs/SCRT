# How to test the v1.11 upgrade with LocalSecret

Always work in docs directory

## Step 1

Start a v1.10.0 chain.

```bash
 docker compose -f docker-compose-111.yml up -d
 docker cp node_init.sh node:/root/
```

On one terminal window:

```bash
docker exec -it bootstrap bash
./bootstrap_init.sh
```

On another terminal window

```bash
docker exec -it node bash
chmod 0777 node_init.sh
./node_init.sh
```

## Step 2 (Test basic contract)

### Copy the suplied contract to the docker

```bash
docker cp ./contract.wasm node:/root/
```

### Access node docker

```bash
docker exec -it node bash
```

### Instantiate a contract and interact with him

```bash
secretd config node http://0.0.0.0:26657
secretd tx compute store contract.wasm --from a --gas 5000000 -y
sleep 5
INIT='{"counter":{"counter":10, "expires":100000}}'
secretd tx compute instantiate 1 "$INIT" --from a --label "c" -y
sleep 5
ADDR=`secretd q compute list-contract-by-code 1 | jq -r '.[0].contract_address'`

secretd tx compute execute $ADDR '{"increment":{"addition": 13}}' --from a -y
sleep 5
secretd query compute query $ADDR '{"get": {}}'
```

Expected result should be:
{"get":{"count":23}}

## Step 4

Propose a software upgrade on the v1.10 chain.

```bash
# 30 blocks (3 minutes) until upgrade block
UPGRADE_BLOCK="$(docker exec node bash -c 'secretd status | jq "(.SyncInfo.latest_block_height | tonumber) + 30"')"

# Propose upgrade
PROPOSAL_ID="$(docker exec node bash -c "secretd tx gov submit-proposal software-upgrade v1.11 --upgrade-height $UPGRADE_BLOCK --title blabla --description yolo --deposit 100000000uscrt --from a -y -b block | jq '.logs[0].events[] | select(.type == \"submit_proposal\") | .attributes[] | select(.key == \"proposal_id\") | .value | tonumber'")"

# Vote yes (voting period is 90 seconds)
docker exec node bash -c "secretd tx gov vote ${PROPOSAL_ID} yes --from a -y -b block"

echo "PROPOSAL_ID   = ${PROPOSAL_ID}"
echo "UPGRADE_BLOCK = ${UPGRADE_BLOCK}"
```

## Step 5

Apply the upgrade.

Wait until you see `ERR CONSENSUS FAILURE!!! err="UPGRADE \"v1.11\" NEEDED at height` in BOTH of the logs,
then, from the root directory of the project, run:

```bash
FEATURES="light-client-validation,random" SGX_MODE=SW make build-linux

# Copy binaries from host to current v1.8 chain

docker exec bootstrap bash -c 'rm -rf /tmp/upgrade-bin && mkdir -p /tmp/upgrade-bin'
docker exec node bash -c 'rm -rf /tmp/upgrade-bin && mkdir -p /tmp/upgrade-bin'

docker cp usr/local/bin/secretd                                    bootstrap:/tmp/upgrade-bin
docker cp usr/lib/librust_cosmwasm_enclave.signed.so           bootstrap:/tmp/upgrade-bin
docker cp usr/lib/libgo_cosmwasm.so                        bootstrap:/tmp/upgrade-bin
docker cp usr/local/bin/secretd                                    node:/tmp/upgrade-bin
docker cp usr/lib/librust_cosmwasm_enclave.signed.so           node:/tmp/upgrade-bin
docker cp usr/lib/libgo_cosmwasm.so                        node:/tmp/upgrade-bin
# These two should be brought from the external repo or from a previous localsecret
docker cp usr/lib/librandom_api.so                                    node:/usr/lib
docker cp usr/lib/tendermint_enclave.signed.so                        node:/usr/lib

docker exec node bash -c 'pkill -9 secretd'

docker exec bootstrap bash -c 'cat /tmp/upgrade-bin/librust_cosmwasm_enclave.signed.so       > /usr/lib/librust_cosmwasm_enclave.signed.so'
docker exec bootstrap bash -c 'cat /tmp/upgrade-bin/libgo_cosmwasm.so                        > /usr/lib/libgo_cosmwasm.so'
docker exec node bash -c 'cat /tmp/upgrade-bin/secretd                                  > /usr/bin/secretd'
docker exec node bash -c 'cat /tmp/upgrade-bin/librust_cosmwasm_enclave.signed.so       > /usr/lib/librust_cosmwasm_enclave.signed.so'
docker exec node bash -c 'cat /tmp/upgrade-bin/libgo_cosmwasm.so                        > /usr/lib/libgo_cosmwasm.so'


rm -rf /tmp/upgrade-bin && mkdir -p /tmp/upgrade-bin
docker cp bootstrap:/root/.secretd/config/priv_validator_key.json /tmp/upgrade-bin/.
docker cp /tmp/upgrade-bin/priv_validator_key.json node:/root/.secretd/config/priv_validator_key.json
```

Then, restart secretd from the node you just killed:

```bash
source /opt/sgxsdk/environment && RUST_BACKTRACE=1 LOG_LEVEL="trace" secretd start --rpc.laddr tcp://0.0.0.0:26657
```

You should see `INF applying upgrade "v1.11" at height` in the logs, following by blocks continute to stream.

## Test that the contract is still there

### Query the value of the counter

```bash
secretd query compute query $ADDR '{"get": {}}'
```

Expected result should be:
{"get":{"count":23}}

## Test contract upgrade
