run:
	export RUST_BACKTRACE=1
	export RUST_LOG=scale::sui=debug
	cargo run -- bot -b sui