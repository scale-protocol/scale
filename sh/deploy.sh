#!/bin/bash

dir=$(dirname $0)
account='0x0e58a921ef201e7f990590a7600d86cb3c614b6b464dc74b58f60509ff6a4f5c'
scale_coin='0xd2e9c858c190f3d83e2d00da12b9a2bc9e4aa3dfca33be0f456e8e97dc47dee4'
# scale_coin='0xc97e6a1bb0f1f98efd7a84075a8c66f09808b2945a5f52ad51d98bbd3eb3a6ca'

if [ "$1" == "coin" ]; then
    echo "airdrop coin"
    scale -g 10000000 sui coin airdrop -a 10000000000
elif [ "$1" == "lsp" ]; then
    echo "create lsp"
    # create lsp and got market_list object id.
    scale -g 10000000 sui trade create_lsp
elif [ "$1" == "scale" ]; then
    echo "init scale package"
    scale -g 10000000 sui trade create_market -s 'Crypto.BTC/USD' -z 1 -o 2000000000 -d 'this is Crypto.BTC/USD trade market' -i 'https://bafybeibicbqm5zwxovveyxanp46njyniixuqn2ic3vv3q5n247qtfnvteu.ipfs.w3s.link/btc.svg'
    scale -g 10000000 sui trade create_market  -s 'Crypto.ETH/USD' -z 1 -o 2000000000 -d 'this is Crypto.ETH/USD trade market' -i 'https://bafybeigphp4mwadmmns34bnwuavplqlbmch22w6krxwdqaemmqmtmvmsna.ipfs.w3s.link/eth.svg'
    # scale -g 10000000 sui trade create_market -s 'Crypto.DOGE/USD' -z 1 -o 2000000000 -d 'this is Crypto.DOGE/USD trade market' -i 'https://bafybeiawqnf5oomm5xouukehulgjdn77vurojopixx6zlxn2tcmepbdlbe.ipfs.w3s.link/doge.svg'
    scale -g 10000000 sui trade create_market -s 'Crypto.SUI/USD' -z 1 -o 2000000000 -d 'this is Crypto.DOGE/USD trade market' -i 'https://bafybeiez2vc7lcgmoudzoxoq7souffwkm6n66uld724j4j5bjudxvfme74.ipfs.w3s.link/sui.svg'
    scale -g 10000000 sui trade add_factory_mould -n 'scale' -d 'default style' -u 'https://gateway.ipfs.io/ipfs/bafybeibckfurkark4hnob2baoayemi7fj24wyrmdct3o45s7qgwijycjyi/1797.png'
    echo "create account"
    scale -g 10000000 sui trade create_account
elif [ "$1" == "oracle" ];then
    echo "init oracle package"
    scale -g 10000000 sui oracle create_price_feed -s Crypto.BTC/USD
    scale -g 10000000 sui oracle create_price_feed -s Crypto.ETH/USD
    # scale -g 10000000 sui oracle create_price_feed -s Crypto.DOGE/USD
    scale -g 10000000 sui oracle create_price_feed -s Crypto.SUI/USD
    scale -g 10000000 sui oracle update_symbol
elif [ "$1" = "deposit" ]
then {
    echo "deposit"
    scale -g 10000000 sui trade deposit -t $account -c $scale_coin -a 5000000000
}
elif [ "$1" = "withdrawal" ]
then {
    echo "withdrawal"
    scale -g 10000000 sui trade withdrawal -t $account -a 1000
}
elif [ "$1" = "investment" ]
then {
    echo "investment"
    timestamp=$(date +%s)
    timestamp=$((timestamp+100000))
    scale -g 10000000 sui trade investment -i $timestamp -c $scale_coin -n 'scale' -a 0
}
elif [ "$1" = "open_cross_position" ]
then {
    # scale -g 100000000 sui oracle update_price_timeout -t 60000
    scale -g 100000000 sui oracle update_pyth_price_bat -f 1 -i 0x50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266
    scale -g 100000000 sui trade open_cross_position -s 'Crypto.SUI/USD' -t $account -l 1 -L 2 -d 1
    scale -g 10000000 sui trade open_cross_position -s 'Crypto.SUI/USD' -t $account -l 1 -L 4 -d 2
    scale -g 10000000 sui trade open_cross_position -s 'Crypto.SUI/USD' -t $account -l 10 -L 2 -d 1 -o 1000.12
    scale -g 10000000 sui trade open_cross_position -s 'Crypto.SUI/USD' -t $account -l 1.2 -L 4 -d 1 -p 1200.12
    scale -g 10000000 sui trade open_cross_position -s 'Crypto.SUI/USD' -t $account -l 1.2 -L 4 -d 1 -p 1200.12 -P 800.12
}
elif [ "$1" = "open_isolated_position" ]
then {
    scale -g 100000000 sui oracle update_pyth_price_bat -f 1 -i 0x50c67b3fd225db8912a424dd4baed60ffdde625ed2feaaf283724f9608fea266
    scale -g 10000000 sui trade open_isolated_position -s 'Crypto.SUI/USD' -t $account -l 10 -L 2 -d 1 -c $scale_coin
}
elif [ "$1" = "close_position" ]
then {
    scale -g 10000000 sui trade investment -i $timestamp -c $scale_coin -n 'scale' -a 0
}
fi