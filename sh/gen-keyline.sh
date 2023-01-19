#！/bin/bash

for file in $(ls ./)
do 
    if [ "${file##*.}" = "csv" ]; then
        echo "handle csv file: $file"
        if [ ! -d "output" ]; then
            mkdir output
        fi
        output_file="output/${file%.*}.csv"
        echo "timestamp,price,feed,conf" > ${output_file}
        awk -F, '{printf "%d,%d,price,0\n",$1,$2*1000000}' $file >> ${output_file}
    fi
done