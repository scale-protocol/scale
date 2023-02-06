run:
	cargo install --path .
	RUST_LOG=debug scale -l /Users/m/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5
debug:
	export RUST_LOG=scale=debug && cargo run -- bot -b sui -e true -d 5