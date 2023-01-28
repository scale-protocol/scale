symbols=("ETHUSDT" "BTCUSDT") # add symbols here to download
# intervals=("1m" "3m" "5m" "15m" "30m" "1h" "2h" "4h" "6h" "8h" "12h" "1d" "3d" "1w" "1mo")
years=("2017" "2018" "2019" "2020" "2021" "2022" "2023")
months=(01 02 03 04 05 06 07 08 09 10 11 12)
intervals=("1m")
# years=("2022")
# months=(12)

baseurl="https://data.binance.vision/data/spot/monthly/klines"

for symbol in ${symbols[@]}; do
  for interval in ${intervals[@]}; do
    for year in ${years[@]}; do
      for month in ${months[@]}; do
        file=${symbol}-${interval}-${year}-${month}.zip
        url="${baseurl}/${symbol}/${interval}/${file}"
        response=$(wget --server-response -q ${url} 2>&1 | awk 'NR==1{print $2}')
        if [ ${response} == '404' ]; then
          echo "File not exist: ${url}" 
        else
          echo "downloaded: ${url}"
          echo "unzip: ${file}"
          unzip ${file}
          rm -rf ${file}
        fi
      done
    done
  done
done  