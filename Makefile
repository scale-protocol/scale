k?=x
build:
	cargo install --path .
run:
	RUST_LOG=scale::sui::subscribe=debug scale -l /Users/lihua/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5 -e true -p 8081 -t 2
debug:
	cargo install --path .
	RUST_LOG=scale::http::service=debug scale -l /Users/lihua/work/lihua/blok/scale/scale/sui.log  bot -b sui -p 8081 -t 2
sub:
	cargo install --path .
	RUST_LOG=scale::sui::subscribe=debug scale  bot -b sui -p 8081 -t 2
config:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- sui config get
call:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- -g 10000000 sui nft mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png'
linux:
	CC_x86_64_unknown_linux_musl="x86_64-linux-musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl
import:
	sui keytool import "$(k)" ed25519 "m/44'/784'/0'/0'/0'"