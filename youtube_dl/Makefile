DIST_DIR := dist

all: install-deps build copy pack

install-deps:
	python -m venv .venv
	ls -l `which sh`
	bash -c "source .venv/bin/activate && pip install git+https://github.com/Moosync/wasm-extension-runner/#subdirectory=wasm-extension-py && pip install --upgrade --force-reinstall git+https://github.com/yt-dlp/yt-dlp && pip install -U yt-dlp-remote-cipher"

build:
	mkdir -p $(DIST_DIR)
	bash -c "source .venv/bin/activate && moosync-edk extension.py -o $(DIST_DIR)/ext.wasm"

opt:
	wasm-opt -Oz $(DIST_DIR)/ext.wasm -o $(DIST_DIR)/ext.wasm

pack:
	mopack --path dist

copy:
	cp package.json dist/

test: build copy
	moodriver ./dist/package.json -vv
