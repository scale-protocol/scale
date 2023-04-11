run:
	cargo install --path .
	RUST_LOG=debug scale -l /Users/m/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5
debug:
	export RUST_LOG=scale::sui=debug && export RUST_BACKTRACE=full && cargo run -- bot -b sui -e false -d 600 -p 8081 -t 10
config:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- sui config get
call:
	export RUST_LOG=scale=debug && export RUST_BACKTRACE=full && cargo run -- -g 10000000 sui nft mint -n 'Scale Invitation' -d 'We hope you will participate in our Testnet' -i 'https://bafybeibkwhppkqtvzagolzcxurtkuu7cvrpcd55eiz4mard3lci2fmyq5y.ipfs.dweb.link/invite.png'
linux:
	CC_x86_64_unknown_linux_musl="x86_64-linux-musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl