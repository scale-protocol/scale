#!/bin/bash
objects=$(sui client objects)
sui_coin=$(grep 'sui::SUI' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
scale_coin=$(grep 'scale::SCALE' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
user_account=$(grep 'account::UserAccount' <<< "$objects" | awk -F '|' '{gsub(/ /,"",$0);print $1}' | head -n1)
account=$(sui client object --json $user_account | jq -r '.data.fields.account_id.fields.id')
# btc_market=(scale sui config get | grep 'scale market list id' | awk '{print $5}' | scale client object | )
market='0x993db2ff6fc3a3af10b7b2af795dc82a01248cbd'

if [ $1 == 'coin' ]
then
    scale sui coin airdrop -c $sui_coin -a 10000000
elif [ $1 == 'scale' ] 
then {
    scale sui contract create_account -c $scale_coin
    scale sui contract create_market -c $scale_coin -s 'Crypto.BTC/USD' -p $scale_coin -i 1 -o 2000000000 -d 'this is Crypto.BTC/USD trade market'
}
else {
    scale sui contract deposit -t $account -c $scale_coin -a 1000000
    scale sui contract add_factory_mould -n 'scale' -d 'default style' -u 'https://gateway.ipfs.io/ipfs/bafybeibckfurkark4hnob2baoayemi7fj24wyrmdct3o45s7qgwijycjyi/1797.png'
    scale sui contract investment -m $market -c $scale_coin -n 'scale'
}
fi