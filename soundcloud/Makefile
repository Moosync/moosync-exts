DIST_DIR := dist

all: build opt copy pack

build:
	GOOS=wasip1 GOARCH=wasm go build -buildmode=c-shared -tags std -o $(DIST_DIR)/ext.wasm .

opt:
	@wasm-opt --enable-bulk-memory -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	@mopack --path dist

copy:
	mkdir -p $(DIST_DIR)
	cp package.json $(DIST_DIR)/

test: build copy
	moodriver -vv -d ./traces $(DIST_DIR)/ext.wasm
