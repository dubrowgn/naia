.PHONY: default
default: build

.PHONY: build
build:
	+cargo build

.PHONY: check
check:
	+RUSTFLAGS="-D warnings" cargo check --all-features --workspace --tests

.PHONY: clean
clean:
	+cargo clean

.PHONY: test
test:
	+RUST_BACKTRACE=1 cargo test --all-features --workspace

.PHONY: pre-push
pre-push: check test

.PHONY: rebuild
rebuild:
	+cargo clean
	+cargo build

