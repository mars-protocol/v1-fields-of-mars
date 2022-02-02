import * as path from "path";
import yargs from "yargs/yargs";
import { MsgMigrateContract } from "@terra-money/terra.js";
import { createLCDClient, createWallet } from "./helpers/cli";
import { storeCodeWithConfirm, sendTxWithConfirm } from "./helpers/tx";

const argv = yargs(process.argv)
  .options({
    network: {
      type: "string",
      demandOption: true,
    },
    contract: {
      type: "string",
      demandOption: true,
    },
    "code-id": {
      type: "number",
      demandOption: false,
    },
  })
  .parseSync();

(async function () {
  const terra = createLCDClient(argv["network"]);
  const admin = createWallet(terra);

  let codeId = argv["code-id"];
  if (!codeId) {
    codeId = await storeCodeWithConfirm(admin, path.resolve("../artifacts/martian_field.wasm"));
    console.log(`Code uploaded! codeId: ${codeId}`);
  }

  const { txhash } = await sendTxWithConfirm(admin, [
    new MsgMigrateContract(admin.key.accAddress, argv["contract"], codeId, {}),
  ]);
  console.log(`Contract migrated! txhash: ${txhash}`);
})();
