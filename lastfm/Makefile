TARGET_DIR = target/wasm32-wasip1/release
DIST_DIR := dist

all: build copy pack

build:
	@cargo build --quiet --release --target wasm32-wasip1

pack:
	@mopack --path .

copy:
	mkdir -p $(DIST_DIR)
	for file in $(TARGET_DIR)/*.wasm; do \
		filename=$$(basename $$file .wasm); \
		cp $$file $(DIST_DIR)/ext.wasm; \
	done

test: build copy
	rm -f ~/.local/share/app.moosync.moosync/extensions/moosync.sample.extension/dist/ext.wasm
	cp $(DIST_DIR)/ext.wasm ~/.local/share/app.moosync.moosync/extensions/moosync.sample.extension/dist
