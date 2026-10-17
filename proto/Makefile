.PHONY: all
all:
	make protogen

.PHONY: protogen
protogen:
	buf generate v1 --template buf.gen.yaml
	python3 generate-modules.py
