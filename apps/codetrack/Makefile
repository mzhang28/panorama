SOURCES := $(shell find -name "*.go")

main.wasm: $(SOURCES)
	GOOS=js GOARCH=wasm go build -o main.wasm
