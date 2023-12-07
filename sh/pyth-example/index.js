"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var pyth_sui_js_1 = require("@pythnetwork/pyth-sui-js");
var sui_js_1 = require("@mysten/sui.js");
var buffer_1 = require("buffer");
var connection = new pyth_sui_js_1.SuiPriceServiceConnection("https://hermes-beta.pyth.network"); // See Hermes endpoints section below for other endpoints
var priceIds = [
    // You can find the ids of prices at https://pyth.network/developers/price-feed-ids
    "0xf9c0172ba10dfa4d19088d94f5bf61d3b54d5bd7483a322a982e1373ee8ea31b", // BTC/USD price id in testnet
    "0xca80ba6dc32e08d06f1aa886011eed1d77c77be9eb761cc10d72b7d0a2fd57a6", // ETH/USD price id in testnet
];
// In order to use Pyth prices in your protocol you need to submit the price update data to Pyth contract in your target
// chain. `getPriceUpdateData` creates the update data which can be submitted to your contract.
var priceFeedUpdateData = await connection.getPriceFeedsUpdateData(priceIds);
var provider = new new sui_js_1.JsonRpcProvider(new sui_js_1.Connection({ fullnode: "" }));
var wallet = new sui_js_1.RawSigner(sui_js_1.Ed25519Keypair.fromSecretKey(buffer_1.Buffer.from(process.env.SUI_KEY, "hex")), provider);
// Get the state ids of the Pyth and Wormhole contracts from
// https://docs.pyth.network/documentation/pythnet-price-feeds/sui#contracts
var wormholeStateId = " 0xFILL_ME";
var pythStateId = "0xFILL_ME";
var client = new pyth_sui_js_1.SuiPythClient(wallet.provider, pythStateId, wormholeStateId);
var tx = new sui_js_1.TransactionBlock();
var priceInfoObjectIds = await client.updatePriceFeeds(tx, priceFeedUpdateData, priceIds);
// tx.moveCall({
//     target: `YOUR_PACKAGE::YOUR_MODULE::use_pyth_for_defi`,
//     arguments: [
//         tx.object(pythStateId),
//         tx.object(priceInfoObjectIds[0]),
//     ],
// });
var txBlock = {
    transactionBlock: tx,
    options: {
        showEffects: true,
        showEvents: true,
    },
};
var result = await wallet.signAndExecuteTransactionBlock(txBlock);
