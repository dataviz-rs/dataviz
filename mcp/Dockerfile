# dataviz-mcp: the dataviz MCP server over stdio.
#
#   docker build -t dataviz-mcp mcp/
#   docker run -i --rm dataviz-mcp
#
# Self-contained: builds the published crate, so it needs no build context.
FROM rust:1.100-slim-bookworm@sha256:bb875b0f8ce726053e259034848c471840e96bfe591fd52556178ae9a05e6126 AS build
RUN cargo install dataviz-mcp --locked --root /out

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
COPY --from=build /out/bin/dataviz-mcp /usr/local/bin/dataviz-mcp
# Charts saved with an `output` path land here; mount a volume to keep them.
WORKDIR /data
ENTRYPOINT ["dataviz-mcp"]
