TARGET_DIR = target/wasm32-wasip1/release
DIST_DIR := dist

all: build copy opt pack

build:
	@cargo update moosync-edk
	@cargo build --quiet --release --target wasm32-wasip1

opt:
	@wasm-opt -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	cp -r assets dist/
	@mopack --path dist

copy:
	mkdir -p $(DIST_DIR)
	for file in $(TARGET_DIR)/*.wasm; do \
		filename=$$(basename $$file .wasm); \
		cp $$file $(DIST_DIR)/ext.wasm; \
	done
	cp package.json dist/

test: build copy
	rm -f ~/.local/share/app.moosync.moosync/extensions/moosync.discord.rpc/dist/ext.wasm
	cp $(DIST_DIR)/ext.wasm ~/.local/share/app.moosync.moosync/extensions/moosync.discord.rpc/dist
