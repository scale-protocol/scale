#!/bin/bash

dir=$(dirname $0)
account='0xcc1e73ed90b498dc8ab2abf83841c529c128a3e3a17bf2e2ba23b7373d56e209'
scale_coin='0x122f5d1f2062f646343c9f7601176d267a9c4cb151e8ae2909efd6d86f6b041f'

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
fi