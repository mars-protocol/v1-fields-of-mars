import { LCDClient } from "@terra-money/terra.js";

/**
 * Query CW20 token balance of a given account
 */
export async function queryCw20Balance(terra: LCDClient, user: string, token: string) {
  const balanceResponse: { balance: string } = await terra.wasm.contractQuery(token, {
    balance: {
      address: user,
    },
  });
  return balanceResponse.balance;
}

/**
 * Query native token balance of a given account
 */
export async function queryNativeBalance(terra: LCDClient, user: string, denom = "uusd") {
  const balance = (await terra.bank.balance(user)).get(denom)?.amount.toString();
  if (balance) {
    return balance;
  } else {
    return "0";
  }
}
