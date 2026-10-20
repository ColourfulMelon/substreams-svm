ENDPOINT ?= solana.substreams.pinax.network:443
START_BLOCK ?= 350000009
STOP_BLOCK ?= +1
PARAMS ?=

.DEFAULT_GOAL := pack

.PHONY: protogen
protogen:
	$(MAKE) -C ../../proto protogen

.PHONY: build
build:
	cargo build --target wasm32-unknown-unknown --release

.PHONY: pack
pack: build
	substreams pack -o ../../spkg/{spkgDefaultName}

.PHONY: run
run: build
	substreams run -e $(ENDPOINT) substreams.yaml map_events -s $(START_BLOCK) -t $(STOP_BLOCK) -p map_events="$(PARAMS)"

.PHONY: gui
gui: build
	substreams gui -e $(ENDPOINT) substreams.yaml map_events -s $(START_BLOCK) -t $(STOP_BLOCK) -p map_events="$(PARAMS)" --network solana
