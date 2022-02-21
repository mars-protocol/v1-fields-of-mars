import BN from "bn.js";

const DECIMAL_FRACTIONAL = new BN("1000000000000000000");

type BigNumberish = BN | number | string;

/**
 * @notice Calculate the output when swapping in an XY-K pool
 */
export function computeXykSwapOutput(
  offerAmount: BigNumberish,
  offerDepth: BigNumberish,
  askDepth: BigNumberish
) {
  offerAmount = new BN(offerAmount);
  offerDepth = new BN(offerDepth);
  askDepth = new BN(askDepth);

  // ask_amount = (ask_pool - cp / (offer_pool + offer_amount))
  //
  // NOTE:
  // 1. when calculating `afterDepthAfter`, Astroport first multiplies `DECIMAL_FRACTIONAL` then
  // divides in the end to offer more precision
  // 2. we assume a 0.3% commission rate
  const cp = offerDepth.mul(askDepth);
  const offerDepthAfter = offerDepth.add(offerAmount);
  const askDepthAfter = cp.mul(DECIMAL_FRACTIONAL).div(offerDepthAfter);
  const returnAmount = askDepth.mul(DECIMAL_FRACTIONAL).sub(askDepthAfter).div(DECIMAL_FRACTIONAL);

  // commission rate = 0.3%
  const commission = returnAmount.mul(new BN(3)).div(new BN(1000));

  // Note: return amount is after deducting commission but before duducting tax
  const returnAmountAfterFee = returnAmount.sub(commission);

  return {
    returnAmount: returnAmount.toString(),
    commission: commission.toString(),
    returnAmountAfterFee: returnAmountAfterFee.toString(),
  };
}

/**
 * @notice Calculate the offer amount required to output a certain asset in an XY-K pool
 */
export function computeXykSwapInput(
  askAmount: BigNumberish,
  offerDepth: BigNumberish,
  askDepth: BigNumberish
) {
  askAmount = new BN(askAmount);
  offerDepth = new BN(offerDepth);
  askDepth = new BN(askDepth);

  // offer_amount = cp / (ask_pool - ask_amount / (1 - commission_rate)) - offer_pool
  //
  // NOTE:
  // 1. when calculating `afterDepthAfter`, Astroport first multiplies `DECIMAL_FRACTIONAL` then
  // divides in the end to offer more precision
  // 2. we assume a 0.3% commission rate
  const cp = offerDepth.mul(askDepth);
  const oneSubFeeRate = DECIMAL_FRACTIONAL.sub(DECIMAL_FRACTIONAL.mul(new BN(3)).div(new BN(1000)));
  const invOneSubFeeRate = DECIMAL_FRACTIONAL.mul(DECIMAL_FRACTIONAL).div(oneSubFeeRate);

  const offerAmount = cp
    .mul(DECIMAL_FRACTIONAL)
    .div(askDepth.sub(askAmount.mul(invOneSubFeeRate).div(DECIMAL_FRACTIONAL)))
    .div(DECIMAL_FRACTIONAL)
    .sub(offerDepth);

  return { offerAmount: offerAmount.toString() };
}
