symbols=("ETHUSDT" "BTCUSDT" "DOGEUSDT")
# symbols=("DOGEUSDT")
# intervals=("1m" "3m" "5m" "15m" "30m" "1h" "2h" "4h" "6h" "8h" "12h" "1d" "3d" "1w" "1mo")
years=("2023")
months=(04)
days=(01 02 03 04 05 06 07 08 09 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31)
intervals=("1m")
# years=("2022")
# months=(12)

baseurl="https://data.binance.vision/data/spot/daily/klines"

for symbol in ${symbols[@]}; do
  for interval in ${intervals[@]}; do
    for year in ${years[@]}; do
      for month in ${months[@]}; do
        for day in ${days[@]}; do
          file=${symbol}-${interval}-${year}-${month}-${day}.zip
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
done  