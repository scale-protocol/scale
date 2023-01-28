#！/bin/bash

if [ ! -d "output" ]; then
    mkdir output
fi
# all_file="output/all.line"
# if [ ! -d "${all_file}" ]; then
#     touch ${all_file}
# fi
echo '' > ${all_file}
for file in $(ls ./)
do 
    echo "handle csv file: $file"
    if [ "${file##*.}" = "csv" ]; then
        coin=$(sed 's/\([A-Z].*\)USDT.*/\1/g' <<< $file)
        coin="Crypto.${coin}/USD"
        output_file="output/${file%.*}.line"
        awk -F, '{printf "%s,feed=price price=%di,conf=0i %d\n","'"${coin}"'",$2*1000000,$1}' $file > ${output_file}
        # awk -F, '{printf "%s,feed=price price=%d,conf=0 %d\n","'"${coin}"'",$2*1000000,$1}' $file >> ${all_file}
    fi
done
