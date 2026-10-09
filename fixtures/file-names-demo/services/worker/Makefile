SHELL := /bin/bash
SERVICE := worker
IMAGE := registry.example.com/shop/$(SERVICE)
VERSION ?= $(shell git describe --tags --always --dirty)

.PHONY: build push test lint clean

build:
	docker build --pull -t $(IMAGE):$(VERSION) .

push: build
	docker push $(IMAGE):$(VERSION)

test:
	npm test -- --reporter=dot

lint:
	npx eslint src --max-warnings=0

clean:
	rm -rf dist coverage
