run:
	cargo install --path .
	RUST_LOG=debug scale -l /Users/m/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5
debug:
	export RUST_LOG=scale::sui=debug && export RUST_BACKTRACE=full && cargo run -- bot -b sui -e true -d 5 -p 8081 -t 10
config:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- sui config get
call:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- sui coin airdrop -c 0x003edccd306483f7348bf20919335f6e5ec8e0caeac6f59142dec81c1959ecd6 -a 3000000000000
linux:
	CC_x86_64_unknown_linux_musl="x86_64-linux-musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl