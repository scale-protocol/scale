#!/bin/bash

if [ "$1" = "mint" ]
then
    scale -g 10000000 sui trade mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png'
fi