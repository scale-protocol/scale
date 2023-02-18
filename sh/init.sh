#!/bin/bash

dir=$(dirname $0)
data=$dir/data
if [ ! -d "$data" ]; then
    mkdir $data
fi
if [ "$1" != "write" ]; then 
sui_coin=$(cat $data/sui_coin)
scale_coin=$(cat $data/scale_coin)
user_account=$(cat $data/user_account)
account=$(cat $data/account)
market=$(cat $data/market)
btc_price_feed=$(cat $data/btc_price_feed)
eth_price_feed=$(cat $data/eth_price_feed)

echo "suicoin: $sui_coin"
echo "scalecoin: $scale_coin"
echo "user_account: $user_account"
echo "account: $account"
echo "market: $market"
fi

if [ "$1" = "write" ]
then
objects=$(sui client objects)
sui_coin=$(grep '::sui::SUI' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
scale_coin=$(grep '0x2::coin::Coin' <<< "$objects"| grep 'scale::SCALE'  | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
user_account=$(grep 'account::UserAccount' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
echo $sui_coin > $data/sui_coin
echo $scale_coin > $data/scale_coin
echo $user_account > $data/user_account
account=$(sui client object --json $user_account | jq -r '.data.fields.account_id.fields.id')
# btc_market=(scale sui config get | grep 'scale market list id' | awk '{print $5}' | scale client object | )
# market='0x3f1a810ddf0b82adf3a3853e405bf4c82952cba7'

echo $account > $data/account
# echo $market > $data/market
ehco $btc_price_feed > $data/btc_price_feed
ehco $eth_price_feed > $data/eth_price_feed

elif [ "$1" = "coin" ]
then
    echo "airdrop coin"
    scale sui coin airdrop -c $sui_coin -a 10000000
elif [ "$1" = "scale" ] 
then {
    echo "create market and account"
    scale sui trade create_account -c $scale_coin
    scale sui trade create_market -c $scale_coin -s 'Crypto.BTC/USD' -p $btc_price_feed -i 1 -o 2000000000 -d 'this is Crypto.BTC/USD trade market'
    scale sui trade create_market -c $scale_coin -s 'Crypto.ETH/USD' -p $eth_price_feed -i 1 -o 2000000000 -d 'this is Crypto.ETH/USD trade market'
    scale sui trade add_factory_mould -n 'scale' -d 'default style' -u 'https://gateway.ipfs.io/ipfs/bafybeibckfurkark4hnob2baoayemi7fj24wyrmdct3o45s7qgwijycjyi/1797.png'
}
elif [ "$1" = "open_position" ]
then {
    echo "open position"
    scale sui trade open_position -m $market -t $account -l 1 -L 2 -p 2 -d 2
}
elif [ "$1" = "close_position" ]
then {
    echo "close position"
    scale sui trade close_position -m $market -t $account -p $2
}
elif [ "$1" = "oracle" ]
then {
    echo "pracle"
    scale sui oracle create_price_feed -s Crypto.BTC/USD
    scale sui oracle create_price_feed -s Crypto.ETH/USD
}
else {
    echo "deposit and investment"
    scale sui trade deposit -t $account -c $scale_coin -a 1000000
    scale sui trade investment -m $market -c $scale_coin -n 'scale'
}
fi