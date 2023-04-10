#!/bin/bash

if [ "$1" = "mint" ]
then
    scale -g 10000000 sui nft mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png'
elif [ "$1" = "mint_to" ]
then
    scale -g 10000000 sui nft mint_recipient -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png' -r $2
elif [ "$1" = "burn" ]
then
    scale -g 10000000 sui nft burn -i $2
else
    echo "usage: sh nft.sh [mint|mint_to|burn]"
fi