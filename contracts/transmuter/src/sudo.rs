use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    ensure, ensure_eq, to_binary, BankMsg, Coin, Decimal, DepsMut, Env, MessageInfo, Response,
    Uint128,
};

use crate::{
    contract::{BurnAlloyedAssetFrom, Transmuter},
    ContractError,
};

#[cw_serde]
pub enum SudoMsg {
    SetActive {
        is_active: bool,
    },
    /// SwapExactAmountIn swaps an exact amount of tokens in for as many tokens out as possible.
    /// The amount of tokens out is determined by the current exchange rate and the swap fee.
    /// The user specifies a minimum amount of tokens out, and the transaction will revert if that amount of tokens
    /// is not received.
    SwapExactAmountIn {
        sender: String,
        token_in: Coin,
        token_out_denom: String,
        token_out_min_amount: Uint128,
        swap_fee: Decimal,
    },
    /// SwapExactAmountOut swaps as many tokens in as possible for an exact amount of tokens out.
    /// The amount of tokens in is determined by the current exchange rate and the swap fee.
    /// The user specifies a maximum amount of tokens in, and the transaction will revert if that amount of tokens
    /// is exceeded.
    SwapExactAmountOut {
        sender: String,
        token_in_denom: String,
        token_in_max_amount: Uint128,
        token_out: Coin,
        swap_fee: Decimal,
    },
}

impl SudoMsg {
    pub fn dispatch(
        self,
        transmuter: &Transmuter,
        ctx: (DepsMut, Env),
    ) -> Result<Response, ContractError> {
        match self {
            SudoMsg::SetActive { is_active } => {
                let (deps, _env) = ctx;
                transmuter.active_status.save(deps.storage, &is_active)?;

                Ok(Response::new().add_attribute("method", "set_active"))
            }
            SudoMsg::SwapExactAmountIn {
                sender,
                token_in,
                token_out_denom,
                token_out_min_amount,
                swap_fee,
            } => {
                let method = "swap_exact_amount_in";

                let (deps, env) = ctx;
                let sender = deps.api.addr_validate(&sender)?;

                let alloyed_denom = transmuter.alloyed_asset.get_alloyed_denom(deps.storage)?;
                let token_out = Coin::new(token_in.amount.u128(), token_out_denom);

                // ensure token_out amount is greater than or equal to token_out_min_amount
                ensure!(
                    token_out.amount >= token_out_min_amount,
                    ContractError::InsufficientTokenOut {
                        required: token_out_min_amount,
                        available: token_out.amount
                    }
                );

                // if token in is share denom, swap alloyed asset for tokens
                if token_in.denom == alloyed_denom {
                    let swap_result = to_binary(&SwapExactAmountInResponseData {
                        token_out_amount: token_out.amount,
                    })?;

                    return transmuter
                        .swap_alloyed_asset_for_tokens(
                            method,
                            BurnAlloyedAssetFrom::SentFunds,
                            (
                                deps,
                                env,
                                MessageInfo {
                                    sender,
                                    funds: vec![token_in],
                                },
                            ),
                            vec![token_out],
                        )
                        .map(|res| res.set_data(swap_result));
                }

                // if token out is share denom, swap token for shares
                if token_out.denom == alloyed_denom {
                    let swap_result = to_binary(&SwapExactAmountInResponseData {
                        token_out_amount: token_out.amount,
                    })?;

                    return transmuter
                        .swap_tokens_for_alloyed_asset(
                            method,
                            (
                                deps,
                                env,
                                MessageInfo {
                                    sender,
                                    funds: vec![token_in],
                                },
                            ),
                        )
                        .map(|res| res.set_data(swap_result));
                }

                let (pool, actual_token_out) = transmuter.do_calc_out_amt_given_in(
                    (deps.as_ref(), env.clone()),
                    token_in,
                    &token_out.denom,
                    swap_fee,
                )?;

                // ensure that actual_token_out is equal to token_out
                // this should never fail
                ensure_eq!(
                    token_out,
                    actual_token_out,
                    ContractError::InvalidTokenOutAmount {
                        expected: token_out.amount,
                        actual: actual_token_out.amount
                    }
                );

                // check and update limiters only if pool assets are not zero
                if let Some(denom_weight_pairs) = pool.weights()? {
                    transmuter.limiters.check_limits_and_update(
                        deps.storage,
                        denom_weight_pairs,
                        env.block.time,
                    )?;
                }

                // save pool
                transmuter.pool.save(deps.storage, &pool)?;

                let send_token_out_to_sender_msg = BankMsg::Send {
                    to_address: sender.to_string(),
                    amount: vec![token_out.clone()],
                };

                let swap_result = SwapExactAmountInResponseData {
                    token_out_amount: token_out.amount,
                };

                Ok(Response::new()
                    .add_attribute("method", method)
                    .add_message(send_token_out_to_sender_msg)
                    .set_data(to_binary(&swap_result)?))
            }
            SudoMsg::SwapExactAmountOut {
                sender,
                token_in_denom,
                token_in_max_amount,
                token_out,
                swap_fee,
            } => {
                let method = "swap_exact_amount_out";
                let (deps, env) = ctx;

                let sender = deps.api.addr_validate(&sender)?;

                let alloyed_denom = transmuter.alloyed_asset.get_alloyed_denom(deps.storage)?;

                let token_in = Coin::new(token_out.amount.u128(), token_in_denom);

                ensure!(
                    token_in.amount <= token_in_max_amount,
                    ContractError::ExcessiveRequiredTokenIn {
                        limit: token_in_max_amount,
                        required: token_in.amount,
                    }
                );

                // if token in is share denom, swap shares for tokens
                if token_in.denom == alloyed_denom {
                    let swap_result = to_binary(&SwapExactAmountOutResponseData {
                        token_in_amount: token_in.amount,
                    })?;

                    return transmuter
                        .swap_alloyed_asset_for_tokens(
                            method,
                            BurnAlloyedAssetFrom::SentFunds,
                            (
                                deps,
                                env,
                                MessageInfo {
                                    sender,
                                    funds: vec![token_in],
                                },
                            ),
                            vec![token_out],
                        )
                        .map(|res| res.set_data(swap_result));
                }

                // if token out is share denom, swap token for shares
                if token_out.denom == alloyed_denom {
                    let swap_result = to_binary(&SwapExactAmountOutResponseData {
                        token_in_amount: token_in.amount,
                    })?;

                    return transmuter
                        .swap_tokens_for_alloyed_asset(
                            method,
                            (
                                deps,
                                env,
                                MessageInfo {
                                    sender,
                                    funds: vec![token_in],
                                },
                            ),
                        )
                        .map(|res| res.set_data(swap_result));
                }

                let (pool, actual_token_in) = transmuter.do_calc_in_amt_given_out(
                    (deps.as_ref(), env.clone()),
                    token_out.clone(),
                    token_in.denom.clone(),
                    swap_fee,
                )?;

                // ensure that actual_token_in is equal to token_in
                // this should never fail
                ensure_eq!(
                    token_in,
                    actual_token_in,
                    ContractError::InvalidTokenInAmount {
                        expected: token_in.amount,
                        actual: actual_token_in.amount
                    }
                );

                // check and update limiters only if pool assets are not zero
                if let Some(denom_weight_pairs) = pool.weights()? {
                    transmuter.limiters.check_limits_and_update(
                        deps.storage,
                        denom_weight_pairs,
                        env.block.time,
                    )?;
                }

                // save pool
                transmuter.pool.save(deps.storage, &pool)?;

                let send_token_out_to_sender_msg = BankMsg::Send {
                    to_address: sender.to_string(),
                    amount: vec![token_out],
                };

                let swap_result = SwapExactAmountOutResponseData {
                    token_in_amount: actual_token_in.amount,
                };

                Ok(Response::new()
                    .add_attribute("method", "swap_exact_amount_out")
                    .add_message(send_token_out_to_sender_msg)
                    .set_data(to_binary(&swap_result)?))
            }
        }
    }
}

#[cw_serde]
/// Fixing token in amount makes token amount out varies
pub struct SwapExactAmountInResponseData {
    pub token_out_amount: Uint128,
}

#[cw_serde]
/// Fixing token out amount makes token amount in varies
pub struct SwapExactAmountOutResponseData {
    pub token_in_amount: Uint128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract::{ContractExecMsg, ExecMsg, InstantiateMsg},
        execute, instantiate, reply, sudo,
    };
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        to_binary, Reply, SubMsgResponse, SubMsgResult,
    };
    use osmosis_std::types::osmosis::tokenfactory::v1beta1::{
        MsgBurn, MsgCreateDenomResponse, MsgMint,
    };

    #[test]
    fn test_swap_exact_amount_in() {
        let mut deps = mock_dependencies();

        let admin = "admin";
        let user = "user";
        let init_msg = InstantiateMsg {
            pool_asset_denoms: vec!["axlusdc".to_string(), "whusdc".to_string()],
            alloyed_asset_subdenom: "uusdc".to_string(),
            admin: Some(admin.to_string()),
        };
        let env = mock_env();
        let info = mock_info(admin, &[]);

        // Instantiate the contract.
        instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();

        // Manually reply
        let alloyed_denom = "uusdc";

        reply(
            deps.as_mut(),
            env.clone(),
            Reply {
                id: 1,
                result: SubMsgResult::Ok(SubMsgResponse {
                    events: vec![],
                    data: Some(
                        MsgCreateDenomResponse {
                            new_token_denom: alloyed_denom.to_string(),
                        }
                        .into(),
                    ),
                }),
            },
        )
        .unwrap();

        let join_pool_msg = ContractExecMsg::Transmuter(ExecMsg::JoinPool {});
        execute(
            deps.as_mut(),
            env.clone(),
            mock_info(
                user,
                &[
                    Coin::new(1_000_000_000_000, "axlusdc"),
                    Coin::new(1_000_000_000_000, "whusdc"),
                ],
            ),
            join_pool_msg,
        )
        .unwrap();

        // Test swap exact amount in with only pool assets
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, "axlusdc".to_string()),
            token_out_denom: "whusdc".to_string(),
            token_out_min_amount: Uint128::from(500u128),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_in")
            .add_message(BankMsg::Send {
                to_address: user.to_string(),
                amount: vec![Coin::new(500, "whusdc".to_string())],
            })
            .set_data(
                to_binary(&SwapExactAmountInResponseData {
                    token_out_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test swap with token in as alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, alloyed_denom),
            token_out_denom: "whusdc".to_string(),
            token_out_min_amount: Uint128::from(500u128),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_in")
            .add_message(MsgBurn {
                amount: Some(Coin::new(500, alloyed_denom).into()),
                sender: env.contract.address.to_string(),
                burn_from_address: env.contract.address.to_string(),
            })
            .add_message(BankMsg::Send {
                to_address: user.to_string(),
                amount: vec![Coin::new(500, "whusdc".to_string())],
            })
            .set_data(
                to_binary(&SwapExactAmountInResponseData {
                    token_out_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test swap with token out as alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, "whusdc".to_string()),
            token_out_denom: alloyed_denom.to_string(),
            token_out_min_amount: Uint128::from(500u128),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_in")
            .add_message(MsgMint {
                sender: env.contract.address.to_string(),
                amount: Some(Coin::new(500, alloyed_denom).into()),
                mint_to_address: user.to_string(),
            })
            .set_data(
                to_binary(&SwapExactAmountInResponseData {
                    token_out_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test case for ensure token_out amount is greater than or equal to token_out_min_amount
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, "whusdc".to_string()),
            token_out_denom: "axlusdc".to_string(),
            token_out_min_amount: Uint128::from(1000u128), // set min amount greater than token_in
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg);

        assert_eq!(
            res,
            Err(ContractError::InsufficientTokenOut {
                required: Uint128::from(1000u128),
                available: Uint128::from(500u128)
            })
        );

        // Test case for ensure token_out amount is greater than or equal to token_out_min_amount but token_in is alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, alloyed_denom.to_string()),
            token_out_denom: "axlusdc".to_string(),
            token_out_min_amount: Uint128::from(1000u128), // set min amount greater than token_in
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg);

        assert_eq!(
            res,
            Err(ContractError::InsufficientTokenOut {
                required: Uint128::from(1000u128),
                available: Uint128::from(500u128)
            })
        );

        // Test case for ensure token_out amount is greater than or equal to token_out_min_amount but token_out is alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountIn {
            sender: user.to_string(),
            token_in: Coin::new(500, "whusdc".to_string()),
            token_out_denom: alloyed_denom.to_string(),
            token_out_min_amount: Uint128::from(1000u128), // set min amount greater than token_in
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env, swap_msg);

        assert_eq!(
            res,
            Err(ContractError::InsufficientTokenOut {
                required: Uint128::from(1000u128),
                available: Uint128::from(500u128)
            })
        );
    }

    #[test]
    fn test_swap_exact_token_out() {
        let mut deps = mock_dependencies();

        let admin = "admin";
        let user = "user";
        let init_msg = InstantiateMsg {
            pool_asset_denoms: vec!["axlusdc".to_string(), "whusdc".to_string()],
            alloyed_asset_subdenom: "uusdc".to_string(),
            admin: Some(admin.to_string()),
        };
        let env = mock_env();
        let info = mock_info(admin, &[]);

        // Instantiate the contract.
        instantiate(deps.as_mut(), env.clone(), info, init_msg).unwrap();

        // Manually reply
        let alloyed_denom = "uusdc";

        reply(
            deps.as_mut(),
            env.clone(),
            Reply {
                id: 1,
                result: SubMsgResult::Ok(SubMsgResponse {
                    events: vec![],
                    data: Some(
                        MsgCreateDenomResponse {
                            new_token_denom: alloyed_denom.to_string(),
                        }
                        .into(),
                    ),
                }),
            },
        )
        .unwrap();

        let join_pool_msg = ContractExecMsg::Transmuter(ExecMsg::JoinPool {});
        execute(
            deps.as_mut(),
            env.clone(),
            mock_info(
                user,
                &[
                    Coin::new(1_000_000_000_000, "axlusdc"),
                    Coin::new(1_000_000_000_000, "whusdc"),
                ],
            ),
            join_pool_msg,
        )
        .unwrap();

        // Test swap exact amount out with only pool assets
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: "axlusdc".to_string(),
            token_in_max_amount: Uint128::from(500u128),
            token_out: Coin::new(500, "whusdc".to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_out")
            .add_message(BankMsg::Send {
                to_address: user.to_string(),
                amount: vec![Coin::new(500, "whusdc".to_string())],
            })
            .set_data(
                to_binary(&SwapExactAmountOutResponseData {
                    token_in_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test swap with token in as alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: alloyed_denom.to_string(),
            token_in_max_amount: Uint128::from(500u128),
            token_out: Coin::new(500, "whusdc".to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_out")
            .add_message(MsgBurn {
                amount: Some(Coin::new(500, alloyed_denom).into()),
                sender: env.contract.address.to_string(),
                burn_from_address: env.contract.address.to_string(),
            })
            .add_message(BankMsg::Send {
                to_address: user.to_string(),
                amount: vec![Coin::new(500, "whusdc".to_string())],
            })
            .set_data(
                to_binary(&SwapExactAmountOutResponseData {
                    token_in_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test swap with token out as alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: "whusdc".to_string(),
            token_in_max_amount: Uint128::from(500u128),
            token_out: Coin::new(500, alloyed_denom.to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg).unwrap();

        let expected = Response::new()
            .add_attribute("method", "swap_exact_amount_out")
            .add_message(MsgMint {
                sender: env.contract.address.to_string(),
                amount: Some(Coin::new(500, alloyed_denom).into()),
                mint_to_address: user.to_string(),
            })
            .set_data(
                to_binary(&SwapExactAmountOutResponseData {
                    token_in_amount: Uint128::from(500u128),
                })
                .unwrap(),
            );

        assert_eq!(res, expected);

        // Test case for ensure token_in amount is less than or equal to token_in_max_amount
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: "whusdc".to_string(),
            token_in_max_amount: Uint128::from(500u128), // set max amount less than token_out
            token_out: Coin::new(1000, "axlusdc".to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg);

        assert_eq!(
            res,
            Err(ContractError::ExcessiveRequiredTokenIn {
                limit: Uint128::from(500u128),
                required: Uint128::from(1000u128),
            })
        );

        // Test case for ensure token_in amount is less than or equal to token_in_max_amount but token_in is alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: alloyed_denom.to_string(),
            token_in_max_amount: Uint128::from(500u128), // set max amount less than token_out
            token_out: Coin::new(1000, "axlusdc".to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env.clone(), swap_msg);

        assert_eq!(
            res,
            Err(ContractError::ExcessiveRequiredTokenIn {
                limit: Uint128::from(500u128),
                required: Uint128::from(1000u128),
            })
        );

        // Test case for ensure token_in amount is less than or equal to token_in_max_amount but token_out is alloyed asset
        let swap_msg = SudoMsg::SwapExactAmountOut {
            sender: user.to_string(),
            token_in_denom: "whusdc".to_string(),
            token_in_max_amount: Uint128::from(500u128), // set max amount less than token_out
            token_out: Coin::new(1000, alloyed_denom.to_string()),
            swap_fee: Decimal::zero(),
        };

        let res = sudo(deps.as_mut(), env, swap_msg);

        assert_eq!(
            res,
            Err(ContractError::ExcessiveRequiredTokenIn {
                limit: Uint128::from(500u128),
                required: Uint128::from(1000u128),
            })
        );
    }
}
