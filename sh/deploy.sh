#!/bin/bash

dir=$(dirname $0)
data=$dir/data
if [ ! -d "$data" ]; then
    mkdir $data
fi

