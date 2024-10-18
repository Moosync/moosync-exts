SUBDIRS := $(shell find . -mindepth 2 -maxdepth 2 -type f -name 'Makefile' -exec dirname {} \;)

.PHONY: all $(SUBDIRS)

all: $(SUBDIRS) copy_msox

DIST_DIR=$(shell pwd)/dist

$(SUBDIRS):
	@echo "Building in $@"
	@$(MAKE) -s -C $@

copy_msox:
	@mkdir -p $(DIST_DIR)
	@echo "Copying all .msox files to $(DIST_DIR)"
	@find . -type f -name '*.msox' -exec mv -f {} $(DIST_DIR) \;
	@echo "All .msox files copied to $(DIST_DIR)"
