DIST_DIR := dist

all: install-deps build copy pack

install-deps:
	python -m venv .venv
	ls -l `which sh`
	source .venv/bin/activate && pip install git+https://github.com/Moosync/wasm-extension-runner/#subdirectory=wasm-extension-py && pip install yt-dlp

build:
	mkdir -p $(DIST_DIR)
	source .venv/bin/activate && moosync-edk extension.py -o $(DIST_DIR)/ext.wasm

opt:
	wasm-opt -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	mopack --path dist

copy:
	cp package.json dist/
