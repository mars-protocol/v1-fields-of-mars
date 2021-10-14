import * as fs from "fs";
import axios from "axios";
import BN from "bn.js";
import {
  isTxError,
  Msg,
  MsgInstantiateContract,
  MsgStoreCode,
  Wallet,
  LCDClient,
} from "@terra-money/terra.js";

export const LOCALTERRA_DEFAULT_GAS_PRICES =
  "0.01133uluna,0.15uusd,0.104938usdr,169.77ukrw,428.571umnt,0.125ueur,0.98ucny,16.37ujpy,0.11ugbp,10.88uinr,0.19ucad,0.14uchf,0.19uaud,0.2usgd,4.62uthb,1.25usek";

/**
 * Fetch the network's minimum gas price of the specified denom
 */
export async function getGasPrice(denom = "uusd", network = "mainnet") {
  // for localterra, we use the default minumum gas price
  if (network === "localterra") {
    const gasPrice = LOCALTERRA_DEFAULT_GAS_PRICES.split(",").find((price) => {
      return price.endsWith(denom);
    });
    if (!gasPrice) {
      throw new Error("invalid fee denom:" + denom);
    }
    // trim off the denom from the end of gasPrice
    const gasPriceValue = gasPrice.substring(0, gasPrice.indexOf(denom));
    return parseFloat(gasPriceValue);
  }

  // for mainnet and testnet, we fetch TFL-recommended gas price from FCD
  // validators don't necessarily use these prices, but let's just assume they do
  let url: string;
  if (network === "mainnet") {
    url = "https://fcd.terra.dev/v1/txs/gas_prices";
  } else if (network === "testnet") {
    url = "https://bombay-fcd.terra.dev/v1/txs/gas_prices";
  } else {
    throw new Error("invalid network: must be `mainnet` | `testnet` | `localterra`");
  }

  type fees = { [key: string]: string };
  const response: { data: fees } = await axios.get(url);
  return parseFloat(response.data[denom]);
}

/**
 * Send a transaction. Return result if successful, throw error if failed
 *
 * Use uusd for gas payment and mainnet gas prices for default. We could customize it to make the
 * function more flexible, but I'm too lazy for that
 */
export async function sendTransaction(terra: LCDClient, sender: Wallet, msgs: Msg[]) {
  const feeDenom = "uusd";
  const network = "mainnet";
  const gasPrice = await getGasPrice(feeDenom, network);
  const tx = await sender.createAndSignTx({
    msgs,
    gasPrices: `${gasPrice}${feeDenom}`,
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
export async function queryCw20Balance(terra: LCDClient, account: string, contract: string) {
  const balanceResponse = await terra.wasm.contractQuery<{ balance: string }>(contract, {
    balance: { address: account },
  });
  return balanceResponse.balance;
}

/**
 * Find native token balance of the specified account
 */
export async function queryNativeBalance(terra: LCDClient, account: string, denom = "uusd") {
  const balance = (await terra.bank.balance(account)).get(denom)?.amount.toString();
  if (balance) {
    return balance;
  } else {
    return "0";
  }
}

/**
 * Encode a JSON object to base64 string
 */
export function toEncodedBinary(obj: any) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}

/**
 * Encode a string to UTF8 array
 */
export function toUtf8Array(str: string) {
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

/**
 * Calculate the output when swapping in a Uniswap V2-style pool
 */
export function computeSwapOutput(
  offerAmount: BN | number | string,
  offerDepth: BN | number | string,
  askDepth: BN | number | string
) {
  offerAmount = new BN(offerAmount);
  offerDepth = new BN(offerDepth);
  askDepth = new BN(askDepth);

  const k = offerDepth.mul(askDepth);
  const askDepthAfter = k.div(offerDepth.add(offerAmount));
  const swapAmount = askDepth.sub(askDepthAfter);

  // commission rate = 0.3%
  const three = new BN("3");
  const thousand = new BN("1000");
  const commission = swapAmount.mul(three).div(thousand);

  // Note: return amount is after deducting commission but before duducting tax
  const returnAmount = swapAmount.sub(commission);

  return {
    swapAmount: swapAmount.toString(),
    returnAmount: returnAmount.toString(),
    commission: commission.toString(),
  };
}
