import * as path from "path";
import yargs from "yargs/yargs";
import { MsgMigrateContract } from "@terra-money/terra.js";
import { createLCDClient, createWallet } from "../helpers/cli";
import { storeCodeWithConfirm, sendTxWithConfirm } from "../helpers/tx";

const argv = yargs(process.argv)
  .options({
    network: {
      type: "string",
      demandOption: true,
    },
    contracts: {
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

  const uploadCode = async () => {
    const codeId = await storeCodeWithConfirm(
      admin,
      path.resolve("../artifacts/martian_field.wasm")
    );
    console.log(`Code uploaded! codeId: ${codeId}`);
    return codeId;
  };
  const codeId = argv["code-id"] ? argv["code-id"] : await uploadCode();

  const msgs = argv["contracts"]
    .split(",")
    .map((addr) => new MsgMigrateContract(admin.key.accAddress, addr, codeId, {}));

  const { txhash } = await sendTxWithConfirm(admin, msgs);
  console.log(`Contract migrated! txhash: ${txhash}`);
})();
