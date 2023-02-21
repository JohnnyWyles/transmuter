/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.24.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type ExecuteMsg = {
  join_pool: {
    [k: string]: unknown;
  };
} | {
  transmute: {
    token_out_denom: string;
    [k: string]: unknown;
  };
} | {
  exit_pool: {
    tokens_out: Coin[];
    [k: string]: unknown;
  };
};
export type Uint128 = string;
export interface Coin {
  amount: Uint128;
  denom: string;
  [k: string]: unknown;
}
export interface InstantiateMsg {
  pool_asset_denoms: string[];
  [k: string]: unknown;
}
export type QueryMsg = {
  pool: {
    [k: string]: unknown;
  };
} | {
  shares: {
    address: string;
    [k: string]: unknown;
  };
};
export interface AdminResponse {
  admin?: string | null;
}
export interface PoolResponse {
  pool: TransmuterPool;
}
export interface TransmuterPool {
  pool_assets: Coin[];
}
export interface SharesResponse {
  shares: Uint128;
}