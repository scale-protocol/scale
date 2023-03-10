#！/bin/bash
DB="https://ifd.scale.exchange"
ORG="scale"
BUCKET="pyth.network"
DB_TOKEN="b-HbA75meluyiNCR16mtC-ybMGUqN6dUG2lux1_i-oFFdKjhiFEJzL5yHcwDo9Fyv8VyRf5mJU4XrO28fmckaA=="
if [ ! -d "$1" ]; then
    echo "please input the path of data"
    exit 1
fi
# write data to influxdb
write_data() {
    echo "write ${1} data to influxdb"
    curl --request POST \
    "${DB}/api/v2/write?org=${ORG}&bucket=${BUCKET}&precision=ms" \
    --header "Authorization: Token ${DB_TOKEN}" \
    --header "Content-Type: text/plain; charset=utf-8" \
    --header "Accept: application/json" \
    --data-binary @${1}
}
write_data_line() {
    echo "write ${1} data to influxdb"
    while read -r line; do
    echo -e "\n line: ${line}"
        curl --request POST \
        "${DB}/api/v2/write?org=${ORG}&bucket=${BUCKET}&precision=ms" \
        --header "Authorization: Token ${DB_TOKEN}" \
        --header "Content-Type: text/plain; charset=utf-8" \
        --header "Accept: application/json" \
        --data-binary "
        ${line}
        "
    done < ${1}
}
for file in $(ls $1)
do 
    if [ "${file##*.}" = "line" ]; then
        echo "handle line file: $file"
        if [ "$2" = "line" ]; then
            write_data_line $1${file}
        else
            write_data $1${file}
        fi
    fi
done