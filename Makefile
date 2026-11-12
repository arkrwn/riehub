# riehub: every command people, agents and CI use. See AGENTS.md.
# `make check` is what CI runs. `make gate` is what a task needs to merge.

.DEFAULT_GOAL := help
DOCS_VENV := .venv-docs

.PHONY: help build fmt lint test rules check security gate tools docs docs-serve clean clean-stack

help:
	@echo "make build       build the project"
	@echo "make fmt         check formatting (no changes made)"
	@echo "make lint        static checks, warnings are errors"
	@echo "make test        run the test suite"
	@echo "make rules       project rules lint (scripts/rules-lint.sh)"
	@echo "make check       fmt + lint + test + rules, what CI runs"
	@echo "make security    dependency and supply-chain checks"
	@echo "make gate        check + security + docs, what the factory requires"
	@echo "make tools       install the tools the gate needs"
	@echo "make docs        strict build of the docs site into site/"
	@echo "make docs-serve  live preview of the docs site"
	@echo "make clean       remove build output"

check: fmt lint test rules

rules:
	./scripts/rules-lint.sh

security:
	./scripts/security.sh

# The factory gate (factory/README.md). It only ever grows: later tasks
# add the web build, the end-to-end suite and backend suites to it.
gate: check security docs

tools:
	./scripts/install-tools.sh

# ---- docs site (MkDocs Material, versions pinned in docs/requirements.txt)

$(DOCS_VENV)/.installed: docs/requirements.txt
	python3 -m venv $(DOCS_VENV)
	$(DOCS_VENV)/bin/python -m pip install -q -r docs/requirements.txt
	@touch $@

docs: $(DOCS_VENV)/.installed
	$(DOCS_VENV)/bin/mkdocs build --strict --site-dir site

docs-serve: $(DOCS_VENV)/.installed
	$(DOCS_VENV)/bin/mkdocs serve

clean: clean-stack
	rm -rf site

# ---- stack: rust
build:
	cargo build --workspace

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

clean-stack:
	cargo clean
