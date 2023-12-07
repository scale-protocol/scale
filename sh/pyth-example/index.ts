import { SuiPriceServiceConnection,SuiPythClient } from "@pythnetwork/pyth-sui-js";
import {
    Connection,
    Ed25519Keypair,
    JsonRpcProvider,
    RawSigner,
    TransactionBlock,
  } from "@mysten/sui.js";
import { Buffer } from "buffer";

const connection = new SuiPriceServiceConnection(
  "https://hermes-beta.pyth.network"
); // See Hermes endpoints section below for other endpoints
 
const priceIds = [
  // You can find the ids of prices at https://pyth.network/developers/price-feed-ids
  "0xf9c0172ba10dfa4d19088d94f5bf61d3b54d5bd7483a322a982e1373ee8ea31b", // BTC/USD price id in testnet
  "0xca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", // ETH/USD price id in testnet
];
 
// In order to use Pyth prices in your protocol you need to submit the price update data to Pyth contract in your target
// chain. `getPriceUpdateData` creates the update data which can be submitted to your contract.
 
const priceFeedUpdateData = await connection.getPriceFeedsUpdateData(priceIds);

const provider = new new JsonRpcProvider(new Connection({ fullnode: "" }));
const wallet = new RawSigner(
    Ed25519Keypair.fromSecretKey(Buffer.from(process.env.SUI_KEY, "hex")),
    provider
  );
// Get the state ids of the Pyth and Wormhole contracts from
// https://docs.pyth.network/documentation/pythnet-price-feeds/sui#contracts
const wormholeStateId = " 0xFILL_ME";
const pythStateId = "0xFILL_ME";
 
const client = new SuiPythClient(wallet.provider, pythStateId, wormholeStateId);
const tx = new TransactionBlock();
const priceInfoObjectIds = await client.updatePriceFeeds(tx, priceFeedUpdateData, priceIds);
 
// tx.moveCall({
//     target: `YOUR_PACKAGE::YOUR_MODULE::use_pyth_for_defi`,
//     arguments: [
//         tx.object(pythStateId),
//         tx.object(priceInfoObjectIds[0]),
//     ],
// });
 
const txBlock = {
    transactionBlock: tx,
    options: {
        showEffects: true,
        showEvents: true,
    },
};
 
const result = await wallet.signAndExecuteTransactionBlock(txBlock);