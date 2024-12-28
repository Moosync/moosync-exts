DIST_DIR := dist

all: build pack

build:
	@corepack yarn install
	@corepack yarn build

opt:
	@wasm-opt -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	@mopack --path .

test: build copy
	rm -f ~/.local/share/app.moosync.moosync/extensions/moosync.sample.extension/dist/ext.wasm
	cp $(DIST_DIR)/ext.wasm ~/.local/share/app.moosync.moosync/extensions/moosync.sample.extension/dist
