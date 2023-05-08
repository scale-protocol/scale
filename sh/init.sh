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
doge_price_feed=$(cat $data/doge_price_feed)

echo "suicoin: $sui_coin"
echo "scalecoin: $scale_coin"
echo "user_account: $user_account"
echo "account: $account"
echo "market: $market"
echo "btc_price_feed: $btc_price_feed"
echo "eth_price_feed: $eth_price_feed"
echo "doge_price_feed: $doge_price_feed"
fi

if [ "$1" = "write" ]
then
objects=$(sui client objects)
sui_coin=$(grep '::sui::SUI' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
# scale_coin=$(grep '0x2::coin::Coin' <<< "$objects"| grep 'scale::SCALE'  | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
# user_account=$(grep 'account::UserAccount' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
echo $sui_coin > $data/sui_coin
# echo $scale_coin > $data/scale_coin
# echo $user_account > $data/user_account
# account=$(sui client object --json $user_account | jq -r '.data.fields.account_id.fields.id')
# btc_market=(scale sui config get | grep 'scale market list id' | awk '{print $5}' | scale client object | )
# market='0x3f1a810ddf0b82adf3a3853e405bf4c82952cba7'

echo $account > $data/account
# echo $market > $data/market
# echo $btc_price_feed > $data/btc_price_feed
# echo $eth_price_feed > $data/eth_price_feed

elif [ "$1" = "coin" ]
then
    echo "airdrop coin"
    scale -g 10000000 sui coin mint -a 9000000000000
    # scale -g 10000000 sui coin airdrop -a 10000000000
elif [ "$1" = "scale" ] 
then {
    echo "create market and account"
    scale -g 10000000 sui trade create_account -c $scale_coin
    scale -g 10000000 sui trade create_market -c $scale_coin -s 'Crypto.BTC/USD' -p $btc_price_feed -z 1 -o 2000000000 -d 'this is Crypto.BTC/USD trade market' -i 'https://bafybeibicbqm5zwxovveyxanp46njyniixuqn2ic3vv3q5n247qtfnvteu.ipfs.w3s.link/btc.svg'
    scale -g 10000000 sui trade create_market -c $scale_coin -s 'Crypto.ETH/USD' -p $eth_price_feed -z 1 -o 2000000000 -d 'this is Crypto.ETH/USD trade market' -i 'https://bafybeigphp4mwadmmns34bnwuavplqlbmch22w6krxwdqaemmqmtmvmsna.ipfs.w3s.link/eth.svg'
    scale -g 10000000 sui trade create_market -c $scale_coin -s 'Crypto.DOGE/USD' -p $doge_price_feed -z 1 -o 2000000000 -d 'this is Crypto.DOGE/USD trade market' -i 'https://bafybeiawqnf5oomm5xouukehulgjdn77vurojopixx6zlxn2tcmepbdlbe.ipfs.w3s.link/doge.svg'
    scale -g 10000000 sui trade add_factory_mould -n 'scale' -d 'default style' -u 'https://gateway.ipfs.io/ipfs/bafybeibckfurkark4hnob2baoayemi7fj24wyrmdct3o45s7qgwijycjyi/1797.png'
}
elif [ "$1" = "open_position" ]
then {
    echo "open position"
    scale -g 10000000 sui trade open_position -m $market -t $account -l 1 -L 2 -p 2 -d 1
}
elif [ "$1" = "close_position" ]
then {
    echo "close position"
    scale sui trade close_position -m $market -t $account -p $2
}
elif [ "$1" = "oracle" ]
then {
    echo "oracle"
    scale -g 10000000 sui oracle create_price_feed -s Crypto.BTC/USD
    scale -g 10000000 sui oracle create_price_feed -s Crypto.ETH/USD
    scale -g 10000000 sui oracle create_price_feed -s Crypto.DOGE/USD
}
elif [ "$1" = "deposit" ]
then {
    echo "deposit"
    scale -g 10000000 sui trade deposit -t $account -c $scale_coin -a 1000
}
elif [ "$1" = "withdrawal" ]
then {
    echo "withdrawal"
    scale -g 10000000 sui trade withdrawal -t $account -a 10000
}
elif [ "$1" = "investment" ]
then {
    echo "investment"
    scale -g 10000000 sui trade investment -m 0x473ed2872a6b8e26650e4feb76572b1cca80c55e2381e084dd7f08170b1fb25d -c $scale_coin -n 'scale' -a 3000000000000
    scale -g 10000000 sui trade investment -m 0x65e3aad55942796208983e003d78b1f8f1867ff0388dc814be80c9dc634265f9 -c $scale_coin -n 'scale' -a 3000000000000
    scale -g 10000000 sui trade investment -m 0x1ec44c3382bece314f07f49b12c53949a02da132db9e65767656a0d4d33be941 -c $scale_coin -n 'scale' -a 0
}
elif [ "$1" = "upprice" ]
then {
    echo "update price"
    scale sui oracle update_price -f $btc_price_feed -p 2000000000
    scale sui trade trigger_update_opening_price -m $market
}
else {
    echo "usage: sh init.sh [write|coin|scale|open_position|close_position|oracle|deposit|withdrawal|upprice|investment]"
}
fi