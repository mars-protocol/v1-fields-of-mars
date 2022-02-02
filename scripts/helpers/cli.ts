import dotenv from "dotenv";
import { LCDClient, LocalTerra, Wallet, MnemonicKey } from "@terra-money/terra.js";

/**
 * @notice Create an `LCDClient` instance based on provided network identifier
 */
export function createLCDClient(network: string): LCDClient {
  if (network === "mainnet") {
    return new LCDClient({
      chainID: "columbus-5",
      URL: "https://lcd.terra.dev",
    });
  } else if (network === "testnet") {
    return new LCDClient({
      chainID: "bombay-12",
      URL: "https://bombay-lcd.terra.dev",
    });
  } else if (network === "localterra") {
    return new LocalTerra();
  } else {
    throw new Error(`invalid network: ${network}, must be mainnet|testnet|localterra`);
  }
}

/**
 * @notice Create a `Wallet` instance by loading the mnemonic phrase stored in `.env`
 */
export function createWallet(terra: LCDClient): Wallet {
  dotenv.config();
  if (!process.env.MNEMONIC) {
    throw new Error("mnemonic not provided");
  }
  return terra.wallet(
    new MnemonicKey({
      mnemonic: process.env.MNEMONIC,
    })
  );
}
