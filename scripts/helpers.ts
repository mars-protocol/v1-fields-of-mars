import * as fs from "fs";
import chalk from "chalk";
import BN from "bn.js";
import {
  isTxError,
  Coin,
  LocalTerra,
  Msg,
  MsgInstantiateContract,
  MsgStoreCode,
  StdFee,
  Wallet,
  LCDClient,
} from "@terra-money/terra.js";

/// Send a transaction. Return result if successful, throw error if failed.
export async function sendTransaction(
  terra: LocalTerra | LCDClient,
  sender: Wallet,
  msgs: Msg[],
  verbose = false
) {
  const GAS_LIMIT = 30000000;
  const GAS_AMOUNT = 4500000;
  const DEFAULT_FEE = new StdFee(GAS_LIMIT, [
    new Coin("uluna", GAS_AMOUNT),
    new Coin("uusd", GAS_AMOUNT),
  ]);

  const tx = await sender.createAndSignTx({ msgs, fee: DEFAULT_FEE });
  const result = await terra.tx.broadcast(tx);

  // Print the log info
  if (verbose) {
    console.log(chalk.magenta("\nTxHash:"), result.txhash);
    try {
      console.log(
        chalk.magenta("Raw log:"),
        JSON.stringify(JSON.parse(result.raw_log), null, 2)
      );
    } catch {
      console.log(chalk.magenta("Failed to parse log! Raw log:"), result.raw_log);
    }
  }

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

/// Upload contract code to LocalTerra. Return code ID.
export async function storeCode(terra: LCDClient, deployer: Wallet, filepath: string) {
  const code = fs.readFileSync(filepath).toString("base64");
  const result = await sendTransaction(terra, deployer, [
    new MsgStoreCode(deployer.key.accAddress, code),
  ]);
  return parseInt(result.logs[0].eventsByType.store_code.code_id[0]);
}

/// Instantiate a contract from an existing code ID. Return contract address
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

/// Find CW20 token balance of the specified account
export async function queryCw20Balance(
  terra: LCDClient,
  account: string,
  contract: string
) {
  const balanceResponse = await terra.wasm.contractQuery<{ balance: string }>(contract, {
    balance: { address: account },
  });
  return balanceResponse.balance;
}

/// Find native token balance of the specified account
export async function queryNativeBalance(
  terra: LocalTerra | LCDClient,
  account: string,
  denom: string = "uusd"
) {
  const balance = (await terra.bank.balance(account)).get(denom)?.amount.toString();
  if (balance) {
    return balance;
  } else {
    return "0";
  }
}

/// Encode a JSON object to base64 string
export function toEncodedBinary(obj: any) {
  return Buffer.from(JSON.stringify(obj)).toString("base64");
}

/// Given a total amount of UST, find the deviverable amount, after tax, if we transfer this amount
/// NOTE: Assumes a tax rate of 0.1% and no tax cap (`must be configured in LocalTerra/config.genesis.json`)
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

/// @notice Given a intended deliverable amount, find the total amount necessary for deliver this amount
/// NOTE: Assumes a tax rate of 0.1% and no tax cap (`must be configured in LocalTerra/config.genesis.json`)
export function addTax(amount: number) {
  const tax = Math.min(new BN(amount).div(new BN(1000)).toNumber(), 1000000);
  return amount + tax;
}

/// @notice Calculate the output when swapping in a Uniswap V2-style pool
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
