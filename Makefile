run:
	export RUST_BACKTRACE=1
	export RUST_LOG=scale=debug
	cargo run -- bot -b sui