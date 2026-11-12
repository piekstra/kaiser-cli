BIN := kp
CARGO ?= cargo

.PHONY: build release test lint fmt fmt-check clean install deps smoke verify dev

# Every target that produces a binary signs it. `cargo build` and
# `cargo install` ad-hoc sign, minting a new code identity each time, and
# macOS scopes keychain "Always Allow" grants to that identity: an unsigned
# rebuild silently revokes the grant and the next run prompts again.
build: SIGN_TARGET = target/debug/$(BIN)
build:
	$(CARGO) build
	@$(SIGN)

release: SIGN_TARGET = target/release/$(BIN)
release:
	$(CARGO) build --release
	@$(SIGN)

test:
	$(CARGO) test

lint:
	$(CARGO) clippy --all-targets -- -D warnings

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt --check

clean:
	$(CARGO) clean

deps:
	$(CARGO) fetch

# `--locked` builds from Cargo.lock, the versions the tests and audit ran on.
install: SIGN_TARGET = $${CARGO_INSTALL_ROOT:-$$HOME/.cargo}/bin/$(BIN)
install:
	$(CARGO) install --path . --locked --force
	@$(SIGN)

dev: SIGN_TARGET = target/debug/$(BIN)
dev:
	$(CARGO) build
	@$(SIGN)

# Offline smoke test on the release binary. It never reads the real keychain:
# KP_KEYCHAIN_SERVICE names a service that holds nothing, and the config
# and state live under target/smoke.
SMOKE_DIR := $(CURDIR)/target/smoke
SMOKE := KP_KEYCHAIN_SERVICE=piekstra.kp.hermetic-tests \
	XDG_CONFIG_HOME=$(SMOKE_DIR)/config \
	./target/release/$(BIN)
smoke: release
	@$(SMOKE) --version >/dev/null
	@$(SMOKE) info --json >/dev/null
	@$(SMOKE) completions bash >/dev/null
	@$(SMOKE) auth status --json >/dev/null
	@$(SMOKE) orgs list --json >/dev/null
	@for c in "orgs list" "orgs search" "auth set-credential" "auth status" \
		"auth logout" "config set" "config show"; do \
		$(SMOKE) $$c --help >/dev/null || exit 1; done
	@echo "smoke ok"

verify: fmt-check lint test smoke

define SIGN
if [ -x "$$HOME/Dev/cli-common/scripts/dev-sign.sh" ]; then \
	"$$HOME/Dev/cli-common/scripts/dev-sign.sh" "$(SIGN_TARGET)"; \
else echo "cli-common/scripts/dev-sign.sh not found — $(SIGN_TARGET) left ad-hoc signed"; fi
endef
