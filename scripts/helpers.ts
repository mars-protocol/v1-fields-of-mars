import * as fs from "fs";
import BN from "bn.js";
import {
  isTxError,
  Msg,
  MsgInstantiateContract,
  MsgStoreCode,
  Wallet,
  LCDClient,
} from "@terra-money/terra.js";

/**
 * Send a transaction. Return result if successful, throw error if failed
 *
 * Use uusd for gas payment and mainnet gas prices for default. We could customize it to make the
 * function more flexible, but I'm too lazy for that
 */
export async function sendTransaction(terra: LCDClient, sender: Wallet, msgs: Msg[]) {
  const feeDenom = "uusd";
  const network = "mainnet";
  const tx = await sender.createAndSignTx({
    msgs,
    gasPrices: "0.15uusd",
    gasAdjustment: 1.4,
  });
  const result = await terra.tx.broadcast(tx);

  if (isTxError(result)) {
    throw new Error("transaction failed! raw log: " + result.raw_log);
  }
  return result;
}

/**
 * Upload contract code to LocalTerra, return code ID
 */
export async function storeCode(terra: LCDClient, deployer: Wallet, filepath: string) {
  const code = fs.readFileSync(filepath).toString("base64");
  const result = await sendTransaction(terra, deployer, [
    new MsgStoreCode(deployer.key.accAddress, code),
  ]);
  return parseInt(result.logs[0].eventsByType.store_code.code_id[0]);
}

/**
 * Instantiate a contract from an existing code ID, return the result
 *
 * Some contract returns different logs for deployment, so the result needs to be parsed on a
 * case-by-case to find out the contract address
 */
export async function instantiateContract(
  terra: LCDClient,
  deployer: Wallet,
  admin: Wallet | undefined, // leave this emtpy then contract is not migratable
  codeId: number,
  initMsg: object
) {
  const result = await sendTransaction(terra, deployer, [
    new MsgInstantiateContract(
      deployer.key.accAddress,
      admin ? admin.key.accAddress : undefined,
      codeId,
      initMsg
    ),
  ]);
  return result;
}

/**
 * Find CW20 token balance of the specified account
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
 * Find native token balance of the specified account
 */
export async function queryNativeBalance(terra: LCDClient, user: string, denom = "uusd") {
  const balance = (await terra.bank.balance(user)).get(denom)?.amount.toString();
  if (balance) {
    return balance;
  } else {
    return "0";
  }
}

/**
 * Encode a JSON object to base64 string
 */
export function encodeBase64(obj: any) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}

/**
 * Encode a string to UTF8 array
 */
export function encodeUtf8(str: string) {
  const encoder = new TextEncoder();
  return Array.from(encoder.encode(str));
}

/**
 * Given a total amount of UST, find the deviverable amount, after tax, if we transfer this amount
 *
 * NOTE: Assumes a tax rate of 0.1% and a cap of 1000000 (must be configured in LocalTerra/config.genesis.json)
 */
export function deductTax(amount: number) {
  const DECIMAL_FRACTION = new BN("1000000000000000000");
  const tax = Math.min(
    amount -
      new BN(amount)
        .mul(DECIMAL_FRACTION)
        .div(DECIMAL_FRACTION.div(new BN(1000)).add(DECIMAL_FRACTION))
        .toNumber(),
    1000000
  );
  return amount - tax;
}

/**
 * Given a intended deliverable amount of UST, find the total amount necessary for deliver this amount
 *
 * NOTE: Assumes a tax rate of 0.1% and a cap of 1000000 (must be configured in LocalTerra/config.genesis.json)
 */
export function addTax(amount: number) {
  const tax = Math.min(new BN(amount).div(new BN(1000)).toNumber(), 1000000);
  return amount + tax;
}
