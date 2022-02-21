import * as fs from "fs";
import * as path from "path";
import yargs from "yargs/yargs";
import { createLCDClient, createWallet } from "../helpers/cli";
import { storeCodeWithConfirm, instantiateWithConfirm } from "../helpers/tx";

const argv = yargs(process.argv)
  .options({
    network: {
      type: "string",
      demandOption: true,
    },
    msg: {
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
  const deployer = createWallet(terra);
  const msg = JSON.parse(fs.readFileSync(path.resolve(argv["msg"]), "utf8"));

  let codeId = argv["code-id"];
  if (!codeId) {
    codeId = await storeCodeWithConfirm(deployer, path.resolve("../artifacts/martian_field.wasm"));
    console.log(`Code uploaded! codeId: ${codeId}`);
  }

  const result = await instantiateWithConfirm(deployer, codeId, msg);
  const address = result.logs[0].eventsByType.instantiate_contract.contract_address[0];
  console.log(`Contract instantiated! address: ${address}`);
})();
