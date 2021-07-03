import * as fs from "fs";
import chalk from "chalk";
import BN from "bn.js";
import {
  isTxError,
  Coin,
  Coins,
  LocalTerra,
  Msg,
  MsgInstantiateContract,
  MsgStoreCode,
  StdFee,
  Wallet,
  LCDClient,
} from "@terra-money/terra.js";

export const GAS_LIMIT = 30000000;
export const GAS_AMOUNT = 4500000;
export const DECIMAL_FRACTION = new BN("1_000_000_000_000_000_000");

/**
 * @notice Send a transaction. Return result if successful, throw error if failed.
 */
export async function sendTransaction(
  terra: LocalTerra | LCDClient,
  sender: Wallet,
  msgs: Msg[]
) {
  const tx = await sender.createAndSignTx({
    msgs,
    fee: new StdFee(GAS_LIMIT, [
      new Coin("uluna", GAS_AMOUNT),
      new Coin("uusd", GAS_AMOUNT),
    ]),
  });
  const result = await terra.tx.broadcast(tx);
  if (isTxError(result)) {
    throw new Error(
      chalk.red("Transaction failed!") +
        `\n${chalk.yellow("code")}: ${result.code}` +
        `\n${chalk.yellow("codespace")}: ${result.codespace}` +
        `\n${chalk.yellow("raw_log")}: ${result.raw_log}`
    );
  }
  return result;
}

/**
 * @notice Upload contract code to LocalTerra. Return code ID.
 */
export async function storeCode(
  terra: LocalTerra | LCDClient,
  deployer: Wallet,
  filepath: string
) {
  const code = fs.readFileSync(filepath).toString("base64");
  const result = await sendTransaction(terra, deployer, [
    new MsgStoreCode(deployer.key.accAddress, code),
  ]);
  return parseInt(result.logs[0].eventsByType.store_code.code_id[0]);
}

/**
 * @notice Instantiate a contract from an existing code ID. Return contract address.
 */
export async function instantiateContract(
  terra: LocalTerra | LCDClient,
  deployer: Wallet,
  codeId: number,
  initMsg: object,
  initCoins?: Coins,
  migratable?: boolean
) {
  const result = await sendTransaction(terra, deployer, [
    new MsgInstantiateContract(
      deployer.key.accAddress,
      codeId,
      initMsg,
      initCoins,
      migratable
    ),
  ]);
  return result;
}

/**
 * @notice Return the native token balance of the specified account
 */
export async function queryNativeTokenBalance(
  terra: LocalTerra | LCDClient,
  account: string,
  denom: string = "uusd"
) {
  const balance = (await terra.bank.balance(account)).get(denom)?.amount.toString();
  if (balance) {
    return balance;
  } else {
    throw Error("Failed to query native token balance");
  }
}

/**
 * @notice Return CW20 token balance of the specified account
 */
export async function queryTokenBalance(
  terra: LocalTerra | LCDClient,
  account: string,
  contract: string
) {
  const balanceResponse = await terra.wasm.contractQuery<{ balance: string }>(contract, {
    balance: { address: account },
  });
  return balanceResponse.balance;
}

/**
 * @notice Given a total amount of UST, find the deviverable amount, after tax, if we
 * transfer this amount.
 * @param amount The total amount
 * @dev Assumes a tax rate of 0.001 and cap of 1000000 uusd.
 * @dev Assumes transferring UST. Transferring LUNA does not incur tax.
 */
export function deductTax(amount: number) {
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
 * @notice Given a intended deliverable amount, find the total amount, including tax,
 * necessary for deliver this amount. Opposite operation of `deductTax`.
 * @param amount The intended deliverable amount
 * @dev Assumes a tax rate of 0.001 and cap of 1000000 uusd.
 * @dev Assumes transferring UST. Transferring LUNA does not incur tax.
 */
export function addTax(amount: number) {
  const tax = Math.min(new BN(amount).div(new BN(1000)).toNumber(), 1000000);
  return amount + tax;
}

/**
 * @notice Encode a JSON object to base64 binary
 */
export function toEncodedBinary(obj: any) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}
