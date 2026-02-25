# Minimal image with just (~12MB total)
FROM docker.io/alpine:latest
RUN apk add --no-cache just
