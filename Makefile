RUSTFMT_CHANNEL =	nightly-2020-07-26

.PHONY: all
all:
	cargo build --release

.PHONY: fmt
fmt: format

.PHONY: format
format:
	cargo +$(RUSTFMT_CHANNEL) fmt

.PHONY: check
check:
	cargo +$(RUSTFMT_CHANNEL) fmt -- --check
