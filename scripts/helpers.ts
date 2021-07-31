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

// Related to transactions
export const GAS_LIMIT = 30000000;
export const GAS_AMOUNT = 4500000;

// Related to calculation of tax
export const DECIMAL_FRACTION = new BN("1_000_000_000_000_000_000");

// Related to calculation of commissions
export const THREE = new BN("3");
export const THOUSAND = new BN("1000");

// Related to calculation of StableSwap output
export const ITERATIONS = 32;
export const ONE = new BN("1");
export const TWO = new BN("2");
export const AMP = new BN("100");
export const LEVERAGE = AMP.mul(TWO); // leverage = amp * n_coins

/**
 * @notice Send a transaction. Return result if successful, throw error if failed.
 */
export async function sendTransaction(
  terra: LocalTerra | LCDClient,
  sender: Wallet,
  msgs: Msg[],
  verbose = false
) {
  const tx = await sender.createAndSignTx({
    msgs,
    fee: new StdFee(GAS_LIMIT, [
      new Coin("uluna", GAS_AMOUNT),
      new Coin("uusd", GAS_AMOUNT),
    ]),
  });

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
  admin: Wallet, // leave this emtpy then contract is not migratable
  codeId: number,
  initMsg: object
) {
  const result = await sendTransaction(terra, deployer, [
    new MsgInstantiateContract(
      deployer.key.accAddress,
      admin.key.accAddress,
      codeId,
      initMsg
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
    return "0";
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

/**
 * @notice Calculate the output when swapping in a Uniswap V2-style pool
 */
export function computeXykSwapOutput(
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
  const commission = swapAmount.mul(THREE).div(THOUSAND);

  // Note: return amount is after deducting commission but before duducting tax
  const returnAmount = swapAmount.sub(commission);

  return {
    swapAmount: swapAmount.toString(),
    returnAmount: returnAmount.toString(),
    commission: commission.toString(),
  };
}

/**
 * @notice Calculate the output when swapping in a Curve V1-style pool
 */
export function computeStableSwapOutput(
  offerAmount: BN | number | string,
  offerDepth: BN | number | string,
  askDepth: BN | number | string
) {
  offerAmount = new BN(offerAmount);
  offerDepth = new BN(offerDepth);
  askDepth = new BN(askDepth);

  const d = _computeD(offerDepth, askDepth);
  const askDepthAfter = _computeNewBalanceOut(offerDepth.add(offerAmount), d);
  const swapAmount = askDepth.sub(askDepthAfter);

  // commission rate = 0.3%
  const commission = swapAmount.mul(THREE).div(THOUSAND);

  // Note: return amount is after deducting commission but before duducting tax
  const returnAmount = swapAmount.sub(commission);

  return {
    swapAmount: swapAmount.toString(),
    returnAmount: returnAmount.toString(),
    commission: commission.toString(),
  };
}

/**
 * @notice Given asset amounts in a stable swap pool, calculate D iteratively
 */
function _computeD(amountA: BN, amountB: BN) {
  const amountATimesCoins = amountA.mul(TWO).add(ONE);
  const amountBTimesCoins = amountB.mul(TWO).add(ONE);
  const sumX = amountA.add(amountB);

  let d = sumX;
  let dPrev: BN;

  for (let i = 0; i < ITERATIONS; i++) {
    dPrev = d;

    let dProd = d;
    dProd = dProd.mul(d).div(amountATimesCoins);
    dProd = dProd.mul(d).div(amountBTimesCoins);

    // step
    const leverageMul = LEVERAGE.mul(sumX);
    const dProdMul = dProd.mul(TWO);
    const lVal = leverageMul.add(dProdMul).mul(d);
    const leverageSub = LEVERAGE.sub(ONE).mul(d);
    const nCoinsSum = dProd.mul(TWO.add(ONE));
    const rVal = leverageSub.add(nCoinsSum);

    d = lVal.div(rVal);

    if (d.eq(dPrev)) break;
  }

  return d;
}

/**
 * @notice Given D and final depth of offer asset (depth before the swap + offer amount),
 * calculate final depth of ask asset (depth before the swap - swap amount)
 */
function _computeNewBalanceOut(newBalanceIn: BN, d: BN) {
  const nom = d.pow(TWO.add(ONE));
  const denom = new BN(newBalanceIn).mul(TWO.pow(TWO)).mul(LEVERAGE);
  const c = nom.div(denom);

  const b = new BN(newBalanceIn).add(d.div(LEVERAGE));

  let y = d;
  let yPrev: BN;

  for (let i = 0; i < ITERATIONS; i++) {
    yPrev = y;

    const nom = y.pow(TWO).add(c);
    const denom = y.mul(TWO).add(b).sub(d);

    y = nom.div(denom);

    if (y.eq(yPrev)) break;
  }

  return y;
}
