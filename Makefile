prefix=${HOME}/.local
BINDIR=${HOME}/bin

all: target/release/musicctl

install: target/release/musicctl
	install -m 0755 $< ${BINDIR}/

target/release/musicctl: src/*.rs Cargo.*
	cargo build --release

test:
	cargo clippy --all
	cargo test

