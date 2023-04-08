#!/bin/bash

if [ "$1" = "mint" ]
then
    scale sui trade mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y/invite.png'
fi