.PHONY: build check fmt test install-local install release clean

build:
	cargo build --release

check:
	cargo check

fmt:
	cargo fmt --all

test:
	cargo test

install-local: build
	mkdir -p ~/.local/bin
	cp target/release/mon ~/.local/bin/
	@echo "installed: ~/.local/bin/mon"

install: build
	sudo cp target/release/mon /usr/local/bin/
	@echo "installed: /usr/local/bin/mon"

release:
	scripts/release.sh

clean:
	cargo clean
	rm -f ~/.local/bin/mon 2>/dev/null || true
