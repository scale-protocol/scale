run:
	cargo install --path .
	RUST_LOG=debug scale -l /Users/m/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5
debug:
	export RUST_LOG=scale::bot::ws=debug && export RUST_BACKTRACE=full && cargo run -- bot -b sui -e true -d 0 -p 8080 -t 10
linux:
	CC_x86_64_unknown_linux_musl="x86_64-linux-musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl