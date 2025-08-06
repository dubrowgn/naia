.PHONY: default
default: build

FLAGS=--all-features --workspace --tests

.PHONY: build
build:
	+cargo build $(FLAGS)

.PHONY: check
check:
	+RUSTFLAGS="-D warnings" cargo check $(FLAGS)

.PHONY: clean
clean:
	+cargo clean

.PHONY: test
test:
	+RUST_BACKTRACE=1 cargo test $(FLAGS) 

.PHONY: pre-push
pre-push: check test

.PHONY: rebuild
rebuild:
	+cargo clean
	+cargo build

