import BN from "bn.js";

const ITERATIONS = 32;
const ZERO = new BN("0");
const ONE = new BN("1");
const TWO = new BN("2");

/**
 * Given the leverage and asset depths of a StableSwap pool, calculate invariant D.
 * @param leverage equals amplification coefficient times the number of coins
 * @param amountA amount of the 1st asset in the pool
 * @param amountB amount of the 2nd asset in the pool
 */
export function computeD(leverage: string, amountA: string, amountB: string) {
  if (amountA == "0" && amountB == "0") return ZERO;

  const amountATimesCoins = new BN(amountA).mul(TWO).add(ONE);
  const amountBTimesCoins = new BN(amountB).mul(TWO).add(ONE);

  const sumX = new BN(amountA).add(new BN(amountB));

  let d = sumX;
  let dPrev: BN;

  for (let i = 0; i < ITERATIONS; i++) {
    dPrev = d;

    let dProd = d;
    dProd = dProd.mul(d).div(amountATimesCoins);
    dProd = dProd.mul(d).div(amountBTimesCoins);

    // step
    const leverageMul = new BN(leverage).mul(sumX);
    const dProdMul = dProd.mul(TWO);
    const lVal = leverageMul.add(dProdMul).mul(d);
    const leverageSub = new BN(leverage).sub(ONE).mul(d);
    const nCoinsSum = dProd.mul(TWO.add(ONE));
    const rVal = leverageSub.add(nCoinsSum);

    d = lVal.div(rVal);

    console.log(`i=${i}, d=${d}`);

    if (d.eq(dPrev)) break;
  }

  return d;
}

export function computeNewBalanceOut(leverage: string, newBalanceIn: string, d: BN) {
  const nom = d.pow(TWO.add(ONE));
  const denom = new BN(newBalanceIn).mul(TWO.pow(TWO)).mul(new BN(leverage));
  const c = nom.div(denom);

  const b = new BN(newBalanceIn).add(d.div(new BN(leverage)));

  let y = d;
  let yPrev: BN;

  for (let i = 0; i < ITERATIONS; i++) {
    yPrev = y;

    const nom = y.pow(TWO).add(c);
    const denom = y.mul(TWO).add(b).sub(d);

    y = nom.div(denom);

    console.log(`i=${i}, y=${y}`);

    if (y.eq(yPrev)) break;
  }

  return y;
}

if (require.main == module) {
  // Assume bLUNA-LUNA pool
  // pool ubluna  698879752552
  // pool uluna   722159275787
  //
  // Swap 100,000 bLUNA for LUNA
  // new balance in = 698879752552 + 100000000000 = 798879752552
  console.log("Calculating D...");
  const d = computeD("200", "698879752552", "722159275787");

  console.log("Calculating newBalanceOut...");
  const newBalanceOut = computeNewBalanceOut("200", "798879752552", d);

  const swapAmount = new BN("722159275787").sub(newBalanceOut);
  console.log("swapAmount:", swapAmount.toString());

  // commission rate = 0.3%
  const commissionAmount = swapAmount.mul(new BN("3")).div(new BN("1000"));
  console.log("commissionAmount:", commissionAmount.toString());

  // return amount = deduceTax(swapAmount - commissionAmount)
  // Note, there is no tax for LUNA transfers
  const returnAmount = swapAmount.sub(commissionAmount);
  console.log("returnAmount:", returnAmount.toString());
}
