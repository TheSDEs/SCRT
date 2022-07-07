use core::time;
use mem::MaybeUninit;
use std::{mem, thread};

use cosmwasm_std::{
    attr, coins, entry_point, to_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, QueryRequest, Reply, ReplyOn, Response, StdError, StdResult, Storage, SubMsg,
    SubMsgResult, WasmMsg, WasmQuery,
};
use cosmwasm_storage::PrefixedStorage;
use secp256k1::Secp256k1;

use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, QueryRes};
use crate::state::{count, count_read, expiration, expiration_read};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    match msg {
        InstantiateMsg::Counter { counter, expires } => {
            if counter == 0 {
                return Err(StdError::generic_err("got wrong counter on init"));
            }

            count(deps.storage).save(&counter)?;
            let expires = env.block.height + expires;
            expiration(deps.storage).save(&expires)?;
            let mut resp = Response::default();
            resp.data = Some(env.contract.address.as_bytes().into());
            Ok(resp)
        }

        // These were ported from the v0.10 test-contract:
        InstantiateMsg::Nop {} => Ok(Response::new().add_attribute("init", "🌈")),
        InstantiateMsg::Callback {
            contract_addr,
            code_hash,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                code_hash,
                contract_addr: contract_addr.clone(),
                msg: Binary::from(r#"{"c":{"x":0,"y":13}}"#.as_bytes().to_vec()),
                funds: vec![],
            }))
            .add_attribute("init with a callback", "🦄")),
        InstantiateMsg::CallbackContractError {
            contract_addr,
            code_hash,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                code_hash,
                msg: Binary::from(r#"{"contract_error":{"error_type":"generic_err"}}"#.as_bytes()),
                funds: vec![],
            }))
            .add_attribute("init with a callback with contract error", "🤷‍♀️")),
        InstantiateMsg::ContractError { error_type } => Err(map_string_to_error(error_type)),
        InstantiateMsg::NoLogs {} => Ok(Response::new()),
        InstantiateMsg::CallbackToInit { code_id, code_hash } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id,
                msg: Binary::from(r#"{"nop":{}}"#.as_bytes().to_vec()),
                code_hash,
                funds: vec![],
                label: String::from("fi"),
            }))
            .add_attribute("instantiating a new contract from init!", "🐙")),
        InstantiateMsg::CallbackBadParams {
            contract_addr,
            code_hash,
        } => Ok(
            Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                code_hash,
                msg: Binary::from(r#"{"c":{"x":"banana","y":3}}"#.as_bytes().to_vec()),
                funds: vec![],
            })),
        ),
        InstantiateMsg::Panic {} => panic!("panic in init"),
        InstantiateMsg::SendExternalQueryDepthCounter {
            to,
            depth,
            code_hash,
        } => Ok(Response::new().add_attribute(
            format!(
                "{}",
                send_external_query_depth_counter(deps.as_ref(), to, depth, code_hash)
            ),
            "",
        )),
        InstantiateMsg::SendExternalQueryRecursionLimit {
            to,
            depth,
            code_hash,
        } => Ok(Response::new().add_attribute(
            "message",
            send_external_query_recursion_limit(deps.as_ref(), to, depth, code_hash)?,
        )),
        InstantiateMsg::CallToInit {
            code_id,
            code_hash,
            label,
            msg,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id,
                code_hash,
                msg: Binary(msg.as_bytes().into()),
                funds: vec![],
                label: label,
            }))
            .add_attribute("a", "a")),
        InstantiateMsg::CallToExec {
            addr,
            code_hash,
            msg,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr,
                code_hash,
                msg: Binary(msg.as_bytes().into()),
                funds: vec![],
            }))
            .add_attribute("b", "b")),
        InstantiateMsg::CallToQuery {
            addr,
            code_hash,
            msg,
        } => {
            let answer: u32 = deps
                .querier
                .query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: addr,
                    code_hash,
                    msg: Binary::from(msg.as_bytes().to_vec()),
                }))
                .map_err(|err| {
                    StdError::generic_err(format!("Got an error from query: {:?}", err))
                })?;

            Ok(Response::new().add_attribute("c", format!("{}", answer)))
        }
    }
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Increment { addition } => increment(deps, addition),
        ExecuteMsg::TransferMoney { amount } => transfer_money(deps, amount),
        ExecuteMsg::RecursiveReply {} => recursive_reply(env, deps),
        ExecuteMsg::RecursiveReplyFail {} => recursive_reply_fail(env, deps),
        ExecuteMsg::InitNewContract {} => init_new_contract(env, deps),
        ExecuteMsg::InitNewContractWithError {} => init_new_contract_with_error(env, deps),

        // These were ported from the v0.10 test-contract:
        ExecuteMsg::A {
            contract_addr,
            code_hash,
            x,
            y,
        } => Ok(a(deps, env, contract_addr, code_hash, x, y)),
        ExecuteMsg::B {
            contract_addr,
            code_hash,
            x,
            y,
        } => Ok(b(deps, env, contract_addr, code_hash, x, y)),
        ExecuteMsg::C { x, y } => Ok(c(deps, env, x, y)),
        ExecuteMsg::UnicodeData {} => Ok(unicode_data(deps, env)),
        ExecuteMsg::EmptyLogKeyValue {} => Ok(empty_log_key_value(deps, env)),
        ExecuteMsg::EmptyData {} => Ok(empty_data(deps, env)),
        ExecuteMsg::NoData {} => Ok(no_data(deps, env)),
        ExecuteMsg::ContractError { error_type } => Err(map_string_to_error(error_type)),
        ExecuteMsg::NoLogs {} => Ok(Response::default()),
        ExecuteMsg::CallbackToInit { code_id, code_hash } => {
            Ok(exec_callback_to_init(deps, env, code_id, code_hash))
        }
        ExecuteMsg::CallbackBadParams {
            contract_addr,
            code_hash,
        } => Ok(exec_callback_bad_params(contract_addr, code_hash)),
        ExecuteMsg::CallbackContractError {
            contract_addr,
            code_hash,
        } => Ok(exec_with_callback_contract_error(contract_addr, code_hash)),
        ExecuteMsg::SetState { key, value } => Ok(set_state(deps, key, value)),
        ExecuteMsg::GetState { key } => Ok(get_state(deps, key)),
        ExecuteMsg::RemoveState { key } => Ok(remove_state(deps, key)),
        ExecuteMsg::TestCanonicalizeAddressErrors {} => test_canonicalize_address_errors(deps),
        ExecuteMsg::Panic {} => panic!("panic in exec"),
        ExecuteMsg::AllocateOnHeap { bytes } => Ok(allocate_on_heap(bytes as usize)),
        ExecuteMsg::PassNullPointerToImportsShouldThrow { pass_type } => {
            Ok(pass_null_pointer_to_imports_should_throw(deps, pass_type))
        }
        ExecuteMsg::SendExternalQuery { to, code_hash } => {
            Ok(Response::new().set_data(vec![send_external_query(deps.as_ref(), to, code_hash)]))
        }
        ExecuteMsg::SendExternalQueryDepthCounter {
            to,
            code_hash,
            depth,
        } => Ok(
            Response::new().set_data(vec![send_external_query_depth_counter(
                deps.as_ref(),
                to,
                depth,
                code_hash,
            )]),
        ),
        ExecuteMsg::SendExternalQueryRecursionLimit {
            to,
            code_hash,
            depth,
        } => Ok(
            Response::new().set_data(to_binary(&send_external_query_recursion_limit(
                deps.as_ref(),
                to,
                depth,
                code_hash,
            )?)?),
        ),
        ExecuteMsg::SendExternalQueryPanic { to, code_hash } => {
            send_external_query_panic(deps, to, code_hash)
        }
        ExecuteMsg::SendExternalQueryError { to, code_hash } => {
            send_external_query_stderror(deps, to, code_hash)
        }
        ExecuteMsg::SendExternalQueryBadAbi { to, code_hash } => {
            send_external_query_bad_abi(deps, to, code_hash)
        }
        ExecuteMsg::SendExternalQueryBadAbiReceiver { to, code_hash } => {
            send_external_query_bad_abi_receiver(deps, to, code_hash)
        }
        ExecuteMsg::LogMsgSender {} => {
            Ok(Response::new().add_attribute("msg.sender", info.sender.as_str()))
        }
        ExecuteMsg::CallbackToLogMsgSender { to, code_hash } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: to.clone(),
                code_hash,
                msg: Binary::from(r#"{"log_msg_sender":{}}"#.as_bytes().to_vec()),
                funds: vec![],
            }))
            .add_attribute("hi", "hey")),
        ExecuteMsg::DepositToContract {} => {
            Ok(Response::new().set_data(to_binary(&info.funds).unwrap()))
        }
        ExecuteMsg::SendFunds {
            amount,
            from: _,
            to,
            denom,
        } => Ok(Response::new().add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: to,
            amount: coins(amount.into(), denom),
        }))),
        ExecuteMsg::SendFundsToInitCallback {
            amount,
            denom,
            code_id,
            code_hash,
        } => Ok(
            Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
                msg: Binary("{\"nop\":{}}".as_bytes().to_vec()),
                code_id,
                code_hash,
                label: String::from("yo"),
                funds: coins(amount.into(), denom),
            })),
        ),
        ExecuteMsg::SendFundsToExecCallback {
            amount,
            denom,
            to,
            code_hash,
        } => Ok(
            Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                msg: Binary("{\"no_data\":{}}".as_bytes().to_vec()),
                contract_addr: to,
                code_hash,
                funds: coins(amount.into(), denom),
            })),
        ),
        ExecuteMsg::Sleep { ms } => {
            thread::sleep(time::Duration::from_millis(ms));

            Ok(Response::new())
        }
        ExecuteMsg::WithFloats { x, y } => Ok(Response::new().set_data(use_floats(x, y))),
        ExecuteMsg::CallToInit {
            code_id,
            code_hash,
            label,
            msg,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
                code_id,
                code_hash,
                msg: Binary(msg.as_bytes().into()),
                funds: vec![],
                label: label,
            }))
            .add_attribute("a", "a")),
        ExecuteMsg::CallToExec {
            addr,
            code_hash,
            msg,
        } => Ok(Response::new()
            .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: addr,
                code_hash: code_hash,
                msg: Binary(msg.as_bytes().into()),
                funds: vec![],
            }))
            .add_attribute("b", "b")),
        ExecuteMsg::CallToQuery {
            addr,
            code_hash,
            msg,
        } => {
            let answer: u32 = deps
                .querier
                .query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: addr,
                    code_hash: code_hash,
                    msg: Binary::from(msg.as_bytes().to_vec()),
                }))
                .map_err(|err| {
                    StdError::generic_err(format!("Got an error from query: {:?}", err))
                })?;

            Ok(Response::new().add_attribute("c", format!("{}", answer)))
        }
        ExecuteMsg::StoreReallyLongKey {} => {
            let mut store = PrefixedStorage::new(deps.storage, b"my_prefix");
            store.set(REALLY_LONG, b"hello");
            Ok(Response::default())
        }
        ExecuteMsg::StoreReallyShortKey {} => {
            let mut store = PrefixedStorage::new(deps.storage, b"my_prefix");
            store.set(b"a", b"hello");
            Ok(Response::default())
        }
        ExecuteMsg::StoreReallyLongValue {} => {
            let mut store = PrefixedStorage::new(deps.storage, b"my_prefix");
            store.set(b"hello", REALLY_LONG);
            Ok(Response::default())
        }
        ExecuteMsg::Secp256k1Verify {
            pubkey,
            sig,
            msg_hash,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res = match deps.api.secp256k1_verify(
                    msg_hash.as_slice(),
                    sig.as_slice(),
                    pubkey.as_slice(),
                ) {
                    Ok(result) => {
                        Ok(Response::new().add_attribute("result", format!("{}", result)))
                    }
                    Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                };
            }

            return res;
        }
        ExecuteMsg::Secp256k1VerifyFromCrate {
            pubkey,
            sig,
            msg_hash,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                let secp256k1_verifier = Secp256k1::verification_only();

                let secp256k1_signature =
                    secp256k1::Signature::from_compact(&sig.0).map_err(|err| {
                        StdError::generic_err(format!("Malformed signature: {:?}", err))
                    })?;
                let secp256k1_pubkey = secp256k1::PublicKey::from_slice(pubkey.0.as_slice())
                    .map_err(|err| StdError::generic_err(format!("Malformed pubkey: {:?}", err)))?;
                let secp256k1_msg =
                    secp256k1::Message::from_slice(&msg_hash.as_slice()).map_err(|err| {
                        StdError::generic_err(format!(
                            "Failed to create a secp256k1 message from signed_bytes: {:?}",
                            err
                        ))
                    })?;

                res = match secp256k1_verifier.verify(
                    &secp256k1_msg,
                    &secp256k1_signature,
                    &secp256k1_pubkey,
                ) {
                    Ok(()) => Ok(Response::new().add_attribute("result", "true")),
                    Err(_err) => Ok(Response::new().add_attribute("result", "false")),
                };
            }

            return res;
        }
        ExecuteMsg::Ed25519Verify {
            pubkey,
            sig,
            msg,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res =
                    match deps
                        .api
                        .ed25519_verify(msg.as_slice(), sig.as_slice(), pubkey.as_slice())
                    {
                        Ok(result) => {
                            Ok(Response::new().add_attribute("result", format!("{}", result)))
                        }
                        Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                    };
            }

            return res;
        }
        ExecuteMsg::Ed25519BatchVerify {
            pubkeys,
            sigs,
            msgs,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res = match deps.api.ed25519_batch_verify(
                    msgs.iter()
                        .map(|m| m.as_slice())
                        .collect::<Vec<&[u8]>>()
                        .as_slice(),
                    sigs.iter()
                        .map(|s| s.as_slice())
                        .collect::<Vec<&[u8]>>()
                        .as_slice(),
                    pubkeys
                        .iter()
                        .map(|p| p.as_slice())
                        .collect::<Vec<&[u8]>>()
                        .as_slice(),
                ) {
                    Ok(result) => {
                        Ok(Response::new().add_attribute("result", format!("{}", result)))
                    }
                    Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                };
            }

            return res;
        }
        ExecuteMsg::Secp256k1RecoverPubkey {
            msg_hash,
            sig,
            recovery_param,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res = match deps.api.secp256k1_recover_pubkey(
                    msg_hash.as_slice(),
                    sig.as_slice(),
                    recovery_param,
                ) {
                    Ok(result) => Ok(Response::new()
                        .add_attribute("result", format!("{}", Binary(result).to_base64()))),
                    Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                };
            }

            return res;
        }
        ExecuteMsg::Secp256k1Sign {
            msg,
            privkey,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res = match deps.api.secp256k1_sign(msg.as_slice(), privkey.as_slice()) {
                    Ok(result) => Ok(Response::new()
                        .add_attribute("result", format!("{}", Binary(result).to_base64()))),
                    Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                };
            }

            return res;
        }
        ExecuteMsg::Ed25519Sign {
            msg,
            privkey,
            iterations,
        } => {
            let mut res = Ok(Response::new());

            // loop for benchmarking
            for _ in 0..iterations {
                res = match deps.api.ed25519_sign(msg.as_slice(), privkey.as_slice()) {
                    Ok(result) => Ok(Response::new()
                        .add_attribute("result", format!("{}", Binary(result).to_base64()))),
                    Err(err) => Err(StdError::generic_err(format!("{:?}", err))),
                };
            }

            return res;
        }
    }
}

pub fn increment(deps: DepsMut, c: u64) -> StdResult<Response> {
    if c == 0 {
        return Err(StdError::generic_err("got wrong counter"));
    }

    let new_count = count_read(deps.storage).load()? + c;
    count(deps.storage).save(&new_count)?;

    let mut resp = Response::default();
    resp.data = Some((new_count as u32).to_be_bytes().into());

    Ok(resp)
}

pub fn transfer_money(_deps: DepsMut, amount: u64) -> StdResult<Response> {
    let mut resp = Response::default();
    resp.messages.push(SubMsg {
        id: 1337,
        msg: CosmosMsg::Bank(BankMsg::Send {
            to_address: "secret105w4vl4gm7q00yg5jngewt5kp7aj0xjk7zrnhw".to_string(),
            amount: coins(amount as u128, "uscrt"),
        }),
        gas_limit: Some(10000000_u64),
        reply_on: ReplyOn::Always,
    });

    Ok(resp)
}

pub fn recursive_reply(env: Env, _deps: DepsMut) -> StdResult<Response> {
    let mut resp = Response::default();
    resp.messages.push(SubMsg {
        id: 1304,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            code_hash: env.contract.code_hash,
            msg: Binary::from("{\"increment\":{\"addition\":2}}".as_bytes().to_vec()),
            funds: vec![],
        }),
        gas_limit: Some(10000000_u64),
        reply_on: ReplyOn::Always,
    });

    Ok(resp)
}

pub fn recursive_reply_fail(env: Env, _deps: DepsMut) -> StdResult<Response> {
    let mut resp = Response::default();
    resp.messages.push(SubMsg {
        id: 1305,
        msg: CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            code_hash: env.contract.code_hash,
            msg: Binary::from("{\"increment\":{\"addition\":0}}".as_bytes().to_vec()),
            funds: vec![],
        }),
        gas_limit: Some(10000000_u64),
        reply_on: ReplyOn::Always,
    });

    Ok(resp)
}

pub fn init_new_contract(env: Env, _deps: DepsMut) -> StdResult<Response> {
    let mut resp = Response::default();
    resp.messages.push(SubMsg {
        id: 1404,
        msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_hash: env.contract.code_hash,
            msg: Binary::from(
                "{\"counter\":{\"counter\":150, \"expires\":100}}"
                    .as_bytes()
                    .to_vec(),
            ),
            funds: vec![],
            label: "new202213".to_string(),
            code_id: 1,
        }),
        gas_limit: Some(10000000_u64),
        reply_on: ReplyOn::Always,
    });

    Ok(resp)
}

pub fn init_new_contract_with_error(env: Env, _deps: DepsMut) -> StdResult<Response> {
    let mut resp = Response::default();
    resp.messages.push(SubMsg {
        id: 1405,
        msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_hash: env.contract.code_hash,
            msg: Binary::from(
                "{\"counter\":{\"counter\":0, \"expires\":100}}"
                    .as_bytes()
                    .to_vec(),
            ),
            funds: vec![],
            label: "new2022133".to_string(),
            code_id: 1,
        }),
        gas_limit: Some(10000000_u64),
        reply_on: ReplyOn::Always,
    });

    Ok(resp)
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Get {} => to_binary(&get(deps, env)?),

        // These were ported from the v0.10 test-contract:
        QueryMsg::ContractError { error_type } => Err(map_string_to_error(error_type)),
        QueryMsg::Panic {} => panic!("panic in query"),
        QueryMsg::ReceiveExternalQuery { num } => {
            Ok(Binary(serde_json_wasm::to_vec(&(num + 1)).unwrap()))
        }
        QueryMsg::SendExternalQueryInfiniteLoop { to, code_hash } => {
            send_external_query_infinite_loop(deps, to, code_hash)
        }
        QueryMsg::WriteToStorage {} => write_to_storage_in_query(deps.storage),
        QueryMsg::RemoveFromStorage {} => remove_from_storage_in_query(deps.storage),
        QueryMsg::SendExternalQueryDepthCounter {
            to,
            depth,
            code_hash,
        } => Ok(to_binary(&send_external_query_depth_counter(
            deps, to, depth, code_hash,
        ))
        .unwrap()),
        QueryMsg::SendExternalQueryRecursionLimit {
            to,
            depth,
            code_hash,
        } => to_binary(&send_external_query_recursion_limit(
            deps, to, depth, code_hash,
        )?),
        QueryMsg::CallToQuery {
            addr,
            code_hash,
            msg,
        } => {
            let answer: u32 = deps
                .querier
                .query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: addr,
                    code_hash: code_hash,
                    msg: Binary::from(msg.as_bytes().to_vec()),
                }))
                .map_err(|err| {
                    StdError::generic_err(format!("Got an error from query: {:?}", err))
                })?;
            return Ok(to_binary(&answer)?);
        }
    }
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> StdResult<Response> {
    match (reply.id, reply.result) {
        (1337, SubMsgResult::Err(_)) => {
            let mut resp = Response::default();
            resp.data = Some(
                (count_read(deps.storage).load()? as u32)
                    .to_be_bytes()
                    .into(),
            );

            Ok(resp)
        }
        (1337, SubMsgResult::Ok(_)) => Err(StdError::generic_err("got wrong bank answer")),
        (1304, SubMsgResult::Err(e)) => Err(StdError::generic_err(format!(
            "recursive reply failed: {}",
            e
        ))),
        (1304, SubMsgResult::Ok(_)) => {
            let mut resp = Response::default();
            resp.data = Some(
                (count_read(deps.storage).load()? as u32)
                    .to_be_bytes()
                    .into(),
            );

            Ok(resp)
        }
        (1305, SubMsgResult::Ok(_)) => {
            Err(StdError::generic_err(format!("recursive reply failed")))
        }
        (1305, SubMsgResult::Err(_)) => {
            let mut resp = Response::default();
            let new_count = 10;
            count(deps.storage).save(&new_count)?;

            resp.data = Some(
                (count_read(deps.storage).load()? as u32)
                    .to_be_bytes()
                    .into(),
            );

            Ok(resp)
        }
        (1404, SubMsgResult::Err(e)) => Err(StdError::generic_err(format!(
            "recursive init failed: {}",
            e
        ))),
        (1404, SubMsgResult::Ok(s)) => match s.data {
            Some(x) => {
                let response = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    code_hash: env.contract.code_hash,
                    contract_addr: String::from_utf8(
                        Binary::from_base64(String::from_utf8(x.to_vec())?.as_str())?.to_vec(),
                    )?,
                    msg: to_binary(&QueryMsg::Get {})?,
                }))?;

                match response {
                    QueryRes::Get { count } => {
                        let mut resp = Response::default();
                        resp.data = Some((count as u32).to_be_bytes().into());
                        return Ok(resp);
                    }
                }
            }
            None => Err(StdError::generic_err(format!(
                "Init didn't response with contract address",
            ))),
        },
        (1405, SubMsgResult::Ok(_)) => Err(StdError::generic_err(format!(
            "recursive init with error failed"
        ))),
        (1405, SubMsgResult::Err(_)) => {
            let mut resp = Response::default();
            let new_count = 1337;
            count(deps.storage).save(&new_count)?;

            resp.data = Some(
                (count_read(deps.storage).load()? as u32)
                    .to_be_bytes()
                    .into(),
            );

            Ok(resp)
        }

        _ => Err(StdError::generic_err("invalid reply id or result")),
    }
}

fn get(deps: Deps, env: Env) -> StdResult<QueryRes> {
    let count = count_read(deps.storage).load()?;
    let expiration = expiration_read(deps.storage).load()?;

    if env.block.height > expiration {
        return Ok(QueryRes::Get { count: 0 });
    }

    Ok(QueryRes::Get { count })
}

fn map_string_to_error(error_type: String) -> StdError {
    let as_str: &str = &error_type[..];
    match as_str {
        "generic_err" => StdError::generic_err("la la 🤯"),
        "invalid_base64" => StdError::invalid_base64("ra ra 🤯"),
        "invalid_utf8" => StdError::invalid_utf8("ka ka 🤯"),
        "not_found" => StdError::not_found("za za 🤯"),
        "parse_err" => StdError::parse_err("na na 🤯", "pa pa 🤯"),
        "serialize_err" => StdError::serialize_err("ba ba 🤯", "ga ga 🤯"),
        // "unauthorized" => StdError::unauthorized(), // dosn't exist in v1
        // "underflow" => StdError::underflow("minuend 🤯", "subtrahend 🤯"), // dosn't exist in v1
        _ => StdError::generic_err("catch-all 🤯"),
    }
}

fn send_external_query_recursion_limit(
    deps: Deps,
    contract_addr: String,
    depth: u8,
    code_hash: String,
) -> StdResult<String> {
    let result = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash.clone(),
            msg: Binary(
                format!(
                    r#"{{"send_external_query_recursion_limit":{{"to":"{}","code_hash":"{}","depth":{}}}}}"#,
                    contract_addr.clone().to_string(),
                    code_hash.clone().to_string(),
                    depth + 1
                )
                .into_bytes(),
            ),
        }));

    // 5 is the current recursion limit.
    if depth != 5 {
        result
    } else {
        match result {
            Err(StdError::GenericErr { msg, .. })
                if msg == "Querier system error: Query recursion limit exceeded" =>
            {
                Ok(String::from("Recursion limit was correctly enforced"))
            }
            _ => Err(StdError::generic_err(
                "Recursion limit was bypassed! this is a bug!",
            )),
        }
    }
}

#[cfg(feature = "with_floats")]
fn use_floats(x: u8, y: u8) -> Binary {
    let res: f64 = (x as f64) / (y as f64);
    to_binary(&format!("{}", res)).unwrap()
}

#[cfg(not(feature = "with_floats"))]
fn use_floats(x: u8, y: u8) -> Binary {
    Binary(vec![x, y])
}

fn send_external_query(deps: Deps, contract_addr: String, code_hash: String) -> u8 {
    let answer: u8 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr,
            code_hash: code_hash,
            msg: Binary::from(r#"{"receive_external_query":{"num":2}}"#.as_bytes().to_vec()),
        }))
        .unwrap();
    answer
}

fn send_external_query_depth_counter(
    deps: Deps,
    contract_addr: String,
    depth: u8,
    code_hash: String,
) -> u8 {
    if depth == 0 {
        return 0;
    }

    let answer: u8 = deps
        .querier
        .query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash.clone(),
            msg: Binary(
                format!(
                    r#"{{"send_external_query_depth_counter":{{"to":"{}","code_hash":"{}","depth":{}}}}}"#,
                    contract_addr.clone(),
                    code_hash.clone(),
                    depth - 1
                )
                .into(),
            ),
        }))
        .unwrap();

    answer + 1
}

fn send_external_query_panic(
    deps: DepsMut,
    contract_addr: String,
    code_hash: String,
) -> StdResult<Response> {
    let err = deps
        .querier
        .query::<u8>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr,
            msg: Binary::from(r#"{"panic":{}}"#.as_bytes().to_vec()),
            code_hash: code_hash,
        }))
        .unwrap_err();

    Err(err)
}

fn send_external_query_stderror(
    deps: DepsMut,
    contract_addr: String,
    code_hash: String,
) -> StdResult<Response> {
    let answer = deps
        .querier
        .query::<Binary>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr,
            msg: Binary::from(
                r#"{"contract_error":{"error_type":"generic_err"}}"#
                    .as_bytes()
                    .to_vec(),
            ),
            code_hash: code_hash,
        }));

    match answer {
        Ok(wtf) => Ok(Response::new().set_data(wtf)),
        Err(e) => Err(e),
    }
}

fn send_external_query_bad_abi(
    deps: DepsMut,
    contract_addr: String,
    code_hash: String,
) -> StdResult<Response> {
    let answer = deps
        .querier
        .query::<Binary>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr,
            code_hash: code_hash,
            msg: Binary::from(
                r#""contract_error":{"error_type":"generic_err"}}"#.as_bytes().to_vec(),
            ),
        }));

    match answer {
        Ok(wtf) => Ok(Response::new().set_data(wtf)),
        Err(e) => Err(e),
    }
}

fn send_external_query_bad_abi_receiver(
    deps: DepsMut,
    contract_addr: String,
    code_hash: String,
) -> StdResult<Response> {
    let answer = deps
        .querier
        .query::<String>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr,
            msg: Binary::from(r#"{"receive_external_query":{"num":25}}"#.as_bytes().to_vec()),
            code_hash: code_hash,
        }));

    match answer {
        Ok(wtf) => Ok(Response::new().add_attribute("wtf", wtf)),
        Err(e) => Err(e),
    }
}

fn exec_callback_bad_params(contract_addr: String, code_hash: String) -> Response {
    Response::new().add_message(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_addr.clone(),
        code_hash: code_hash,
        msg: Binary::from(r#"{"c":{"x":"banana","y":3}}"#.as_bytes().to_vec()),
        funds: vec![],
    }))
}

pub fn a(
    _deps: DepsMut,
    _env: Env,
    contract_addr: String,
    code_hash: String,
    x: u8,
    y: u8,
) -> Response {
    Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash.clone(),
            msg: Binary::from(
                format!(
            "{{\"b\":{{\"x\":{} ,\"y\": {},\"contract_addr\": \"{}\",\"code_hash\": \"{}\" }}}}",
            x,
            y,
            contract_addr.as_str(),
            &code_hash
        )
                .as_bytes()
                .to_vec(),
            ),
            funds: vec![],
        }))
        .add_attribute("banana", "🍌")
        .set_data(vec![x, y])
}

pub fn b(
    _deps: DepsMut,
    _env: Env,
    contract_addr: String,
    code_hash: String,
    x: u8,
    y: u8,
) -> Response {
    Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash,
            msg: Binary::from(
                format!("{{\"c\":{{\"x\":{} ,\"y\": {} }}}}", x + 1, y + 1)
                    .as_bytes()
                    .to_vec(),
            ),
            funds: vec![],
        }))
        .add_attribute("kiwi", "🥝")
        .set_data(vec![x + y])
}

pub fn c(_deps: DepsMut, _env: Env, x: u8, y: u8) -> Response {
    Response::new()
        .add_attribute("watermelon", "🍉")
        .set_data(vec![x + y])
}

pub fn empty_log_key_value(_deps: DepsMut, _env: Env) -> Response {
    Response::new().add_attributes(vec![
        attr("my value is empty", ""),
        attr("", "my key is empty"),
    ])
}

pub fn empty_data(_deps: DepsMut, _env: Env) -> Response {
    Response::new().set_data(vec![])
}

pub fn unicode_data(_deps: DepsMut, _env: Env) -> Response {
    Response::new().set_data("🍆🥑🍄".as_bytes().to_vec())
}

pub fn no_data(_deps: DepsMut, _env: Env) -> Response {
    Response::new()
}

pub fn exec_callback_to_init(
    _deps: DepsMut,
    _env: Env,
    code_id: u64,
    code_hash: String,
) -> Response {
    Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id,
            msg: Binary::from("{\"nop\":{}}".as_bytes().to_vec()),
            code_hash,
            funds: vec![],
            label: String::from("hi"),
        }))
        .add_attribute("instantiating a new contract", "🪂")
}

fn exec_with_callback_contract_error(contract_addr: String, code_hash: String) -> Response {
    Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash,
            msg: Binary::from(
                r#"{"contract_error":{"error_type":"generic_err"}}"#
                    .as_bytes()
                    .to_vec(),
            ),
            funds: vec![],
        }))
        .add_attribute("exec with a callback with contract error", "🤷‍♂️")
}

fn allocate_on_heap(bytes: usize) -> Response {
    let mut values: Vec<u8> = vec![0; bytes];
    values[bytes - 1] = 1;

    Response::new().set_data("😅".as_bytes().to_vec())
}

fn get_state(deps: DepsMut, key: String) -> Response {
    let store = PrefixedStorage::new(deps.storage, b"my_prefix");

    match store.get(key.as_bytes()) {
        Some(value) => Response::new().set_data(value),
        None => Response::default(),
    }
}

fn set_state(deps: DepsMut, key: String, value: String) -> Response {
    let mut store = PrefixedStorage::new(deps.storage, b"my_prefix");
    store.set(key.as_bytes(), value.as_bytes());
    Response::default()
}

fn remove_state(deps: DepsMut, key: String) -> Response {
    let mut store = PrefixedStorage::new(deps.storage, b"my_prefix");
    store.remove(key.as_bytes());
    Response::default()
}

#[allow(invalid_value)]
#[allow(unused_must_use)]
fn pass_null_pointer_to_imports_should_throw(deps: DepsMut, pass_type: String) -> Response {
    let null_ptr_slice: &[u8] = unsafe { MaybeUninit::zeroed().assume_init() };

    match &pass_type[..] {
        "read_db_key" => {
            deps.storage.get(null_ptr_slice);
        }
        "write_db_key" => {
            deps.storage.set(null_ptr_slice, b"write value");
        }
        "write_db_value" => {
            deps.storage.set(b"write key", null_ptr_slice);
        }
        "remove_db_key" => {
            deps.storage.remove(null_ptr_slice);
        }
        "canonicalize_address_input" => {
            deps.api
                .addr_canonicalize(unsafe { MaybeUninit::zeroed().assume_init() });
        }
        "canonicalize_address_output" => { /* TODO */ }
        "humanize_address_input" => {
            deps.api
                .addr_humanize(unsafe { MaybeUninit::zeroed().assume_init() });
        }
        "humanize_address_output" => { /* TODO */ }
        "validate_address_input" => {
            deps.api
                .addr_validate(unsafe { MaybeUninit::zeroed().assume_init() });
        }
        "validate_address_output" => { /* TODO */ }
        _ => {}
    };

    Response::default()
}

fn test_canonicalize_address_errors(deps: DepsMut) -> StdResult<Response> {
    match deps.api.addr_canonicalize("") {
        Err(StdError::GenericErr { msg }) => {
            if msg != String::from("addr_canonicalize errored: Input is empty") {
                return Err(StdError::generic_err(
                    "empty address should have failed with 'addr_canonicalize errored: Input is empty'",
                ));
            }
            // all is good, continue
        }
        _ => {
            return Err(StdError::generic_err(
                "empty address should have failed with 'addr_canonicalize errored: Input is empty'",
            ))
        }
    }

    match deps.api.addr_canonicalize("   ") {
        Err(StdError::GenericErr { msg }) => {
            if msg != String::from("addr_canonicalize errored: invalid length") {
                return Err(StdError::generic_err(
                    "empty trimmed address should have failed with 'addr_canonicalize errored: invalid length'",
                ));
            }
            // all is good, continue
        }
        _ => {
            return Err(StdError::generic_err(
                "empty trimmed address should have failed with 'addr_canonicalize errored: invalid length'",
            ))
        }
    }

    match deps.api.addr_canonicalize("cosmos1h99hrcc54ms9lxxxx") {
        Err(StdError::GenericErr { msg }) => {
            if msg != String::from("addr_canonicalize errored: invalid checksum") {
                return Err(StdError::generic_err(
                    "bad bech32 should have failed with 'addr_canonicalize errored: invalid checksum'",
                ));
            }
            // all is good, continue
        }
        _ => {
            return Err(StdError::generic_err(
                "bad bech32 should have failed with 'addr_canonicalize errored: invalid checksum'",
            ))
        }
    }

    match deps.api.addr_canonicalize("cosmos1h99hrcc54ms9luwpex9kw0rwdt7etvfdyxh6gu") {
        Err(StdError::GenericErr { msg }) => {
            if msg != String::from("addr_canonicalize errored: wrong address prefix: \"cosmos\"")
            {
                return Err(StdError::generic_err(
                    "bad prefix should have failed with 'addr_canonicalize errored: wrong address prefix: \"cosmos\"'",
                    ));
            }
            // all is good, continue
        }
        _ => {
            return Err(StdError::generic_err(
                "bad prefix should have failed with 'addr_canonicalize errored: wrong address prefix: \"cosmos\"'",
            ))
        }
    }

    Ok(Response::new().set_data("🤟".as_bytes().to_vec()))
}

/////////////////////////////// Query ///////////////////////////////

fn send_external_query_infinite_loop(
    deps: Deps,
    contract_addr: String,
    code_hash: String,
) -> StdResult<Binary> {
    let answer = deps
        .querier
        .query::<Binary>(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: contract_addr.clone(),
            code_hash: code_hash.clone(),
            msg: Binary::from(
                format!(
                    r#"{{"send_external_query_infinite_loop":{{"to":"{}", "code_hash":"{}"}}}}"#,
                    contract_addr.clone().to_string(),
                    &code_hash
                )
                .as_bytes()
                .to_vec(),
            ),
        }));

    match answer {
        Ok(wtf) => Ok(Binary(wtf.into())),
        Err(e) => Err(e),
    }
}

fn write_to_storage_in_query(storage: &dyn Storage) -> StdResult<Binary> {
    #[allow(clippy::cast_ref_to_mut)]
    let storage = unsafe { &mut *(storage as *const _ as *mut dyn Storage) };
    storage.set(b"abcd", b"dcba");

    Ok(Binary(vec![]))
}

fn remove_from_storage_in_query(storage: &dyn Storage) -> StdResult<Binary> {
    #[allow(clippy::cast_ref_to_mut)]
    let storage = unsafe { &mut *(storage as *const _ as *mut dyn Storage) };
    storage.remove(b"abcd");

    Ok(Binary(vec![]))
}

//// consts

const REALLY_LONG: &[u8] = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";