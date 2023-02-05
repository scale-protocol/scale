run:
	cargo install --path .
	RUST_LOG=debug scale -l /Users/m/work/lihua/blok/scale/scale/sui.log bot -b sui -d 5
debug:
	export RUST_BACKTRACE=1
	export RUST_LOG=scale::bot::oracle=debug
	cargo run -- bot -b sui -d 5 -e true