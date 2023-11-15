.PHONY: build clean publish

# Variables
DOCKER_IMAGE_NAME ?= switchboardlabs/backfill-oracle-worker

# Default make task
all: build

docker_build:
	docker buildx build --pull --platform linux/amd64 --pull -t ${DOCKER_IMAGE_NAME}:latest .

docker_publish:
	docker buildx build --pull --platform linux/amd64 --pull -t ${DOCKER_IMAGE_NAME}:latest --push .

build: docker_build

publish: docker_publish

# Task to clean up the compiled rust application
clean:
	cargo clean
