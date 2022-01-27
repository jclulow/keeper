.PHONY: all
all:
	cargo build --release

.PHONY: fmt
fmt: format

.PHONY: format
format:
	cargo fmt

.PHONY: check
check:
	cargo fmt -- --check
