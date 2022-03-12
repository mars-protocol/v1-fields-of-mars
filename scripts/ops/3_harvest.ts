import { Msg, MsgExecuteContract } from "@terra-money/terra.js";
import { createLCDClient, createWallet } from "../helpers/cli";
import { sendTxWithConfirm } from "../helpers/tx";

const CONTRACTS = [
  "terra1kztywx50wv38r58unxj9p6k3pgr2ux6w5x68md", // LUNA-UST strategy
  "terra1vapq79y9cqghqny7zt72g4qukndz282uvqwtz6", // ANC-UST strategy
  "terra12dq4wmfcsnz6ycep6ek4umtuaj6luhfp256hyu", // MIR-UST strategy
];

// https://stackoverflow.com/questions/2450954/how-to-randomize-shuffle-a-javascript-array
function shuffle(array: Msg[]) {
  for (let i = array.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [array[i], array[j]] = [array[j], array[i]];
  }
}

(async function () {
  const terra = createLCDClient("mainnet");
  const operator = createWallet(terra);

  const msgs = CONTRACTS.map(
    (contract) =>
      new MsgExecuteContract(operator.key.accAddress, contract, {
        harvest: {
          max_spread: "0.01",
          slippage_tolerance: "0.01",
        },
      })
  );

  // The strategy to be harvested first is able to sell ASTRO at a higher price than strategies
  // harvested later are. To ensure fairness, we shuffle the array of messages every time
  shuffle(msgs);

  const { txhash } = await sendTxWithConfirm(operator, msgs);
  console.log(`Harvested! txhash: ${txhash}`);
})();
