.PHONY: build check fmt test install-local install release clean

build:
	go build -trimpath -ldflags "-s -w" -o bin/mon .

check:
	test -z "$$(gofmt -l .)"
	go vet ./...

fmt:
	gofmt -w .

test:
	go test -race ./...

install-local: build
	mkdir -p ~/.local/bin
	cp bin/mon ~/.local/bin/
	@echo "installed: ~/.local/bin/mon"

install: build
	sudo cp bin/mon /usr/local/bin/
	@echo "installed: /usr/local/bin/mon"

release:
	scripts/release.sh

clean:
	rm -rf bin
	go clean -testcache
	rm -f ~/.local/bin/mon 2>/dev/null || true
