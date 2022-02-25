import { MsgExecuteContract } from "@terra-money/terra.js";
import { createLCDClient, createWallet } from "../helpers/cli";
import { sendTxWithConfirm } from "../helpers/tx";

const CONTRACT_ADDR = "terra16htwvqxqkygazqxmgt9wpqr0vtf6jekm6mf44n";

(async function () {
  const terra = createLCDClient("testnet");
  const user = createWallet(terra);

  const { txhash } = await sendTxWithConfirm(user, [
    new MsgExecuteContract(user.key.accAddress, CONTRACT_ADDR, {
      harvest: {
        max_spread: "0.10",
        slippage_tolerance: "0.10",
      },
    }),
  ]);
  console.log(`Harvested! txhash: ${txhash}`);
})();
