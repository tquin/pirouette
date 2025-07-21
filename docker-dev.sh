#!/usr/bin/env bash
set -eu; set -o pipefail

CONTAINER_NAME="pirouette-dev"
IMAGE_TAG="dev"

container_state=$(docker ps -a -q -f name="$CONTAINER_NAME")
if [[ -n "$container_state" ]]; then
  docker container stop $CONTAINER_NAME
  docker container rm $CONTAINER_NAME
fi

docker build -t "tquin/pirouette:$IMAGE_TAG" .

mkdir -p /tmp/pirouette_source /tmp/pirouette_target
touch /tmp/pirouette_source/foo.txt

docker run \
  --name $CONTAINER_NAME --rm \
  --user $(id -u):$(id -g) \
  -e PIROUETTE_CONFIG_FILE=/config/pirouette.toml \
  -v /tmp/pirouette_source:/source \
  -v /tmp/pirouette_target:/target \
  -v ${PWD}/pirouette.toml:/config/pirouette.toml \
  "tquin/pirouette:$IMAGE_TAG"
