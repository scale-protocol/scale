#！/bin/bash
symbols=("ETHUSDT" "BTCUSDT" "DOGEUSDT")
intervals=("1m")

baseurl="https://fapi.binance.com/fapi/v1/klines"



if [ ! -d "output" ]; then
    mkdir output
fi
for symbol in ${symbols[@]}; do
    coin=$(sed 's/\([A-Z].*\)USDT.*/\1/g' <<< $symbol)
    coin="Crypto.${coin}/USD"
  for interval in ${intervals[@]}; do
    today_time=$(date '+%Y-%m-%d 00:00:00')
    start_time=$(date -j -f '%Y-%m-%d %H:%M:%S' "${today_time}" +%s )
    end_time=$(date +%s)
    while [ ${start_time} -lt ${end_time} ]; do
        limit_time=$((${start_time}+3600))
        req_url="${baseurl}?symbol=${symbol}&interval=${interval}&startTime=${start_time}000&endTime=${limit_time}000&limit=1500"
        data=$(curl -s ${req_url}) 
        json_data=$(jq -c '.[]' <<< ${data})
        output_file="output/${symbol}-${interval}-today.line"
        while read -r item; do
            # echo -e "item: ${item}\n"
            time=$(jq -r '.[0]' <<< ${item})
            price=$(jq -r '.[1]' <<< ${item})
            price=`echo $price*1000000|bc`
            printf "%s,feed=price price=%-4.0fi,conf=0i %d\n" $coin $price $time >> ${output_file}
        done <<< "$json_data"
        start_time=${limit_time}
    done
  done
done