TARGET_DIR = target/wasm32-wasip1/release
DIST_DIR := dist

all: build opt copy pack

build:
	@cargo build --release --target wasm32-wasip1

opt:
	@wasm-opt -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	@mopack --path dist

copy:
	mkdir -p $(DIST_DIR)
	for file in $(TARGET_DIR)/*.wasm; do \
		filename=$$(basename $$file .wasm); \
		cp $$file $(DIST_DIR)/ext.wasm; \
	done
	cp package.json dist/

test: build copy
	moodriver -d ./traces dist/ext.wasm
