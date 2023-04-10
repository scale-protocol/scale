#!/bin/bash

if [ "$1" = "mint" ]
then
    scale -g 10000000 sui trade mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png'
elif [ "$1" == "mint_to"]
then
    scale -g 10000000 sui trade mint_to -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png' -a $2
elif [ "$1" == "transfer"]
then
    scale -g 10000000 sui trade transfer -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png' -a $2 -t $3
elif [ "$1" == "burn"]
then
    scale -g 10000000 sui trade burn -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png' -a $2
else
    echo "usage: sh nft.sh [mint|mint_to|transfer|burn]"
fi