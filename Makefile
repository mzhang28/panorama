deploy-docs:
	mdbook build docs
	rsync -azrP docs/book/ root@veil:/home/blogDeploy/public/panorama

JOURNAL_SOURCES := $(shell find . apps/journal -name "*.rs" -not -path "./target/*")
journal: $(JOURNAL_SOURCES)
	cargo build \
		--profile=wasm-debug \
		-p panorama-journal \
		--target=wasm32-unknown-unknown

test-install-apps: journal
	cargo test -p panorama-core -- tests::test_install_apps