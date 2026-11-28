# dataviz-mcp: the dataviz MCP server over stdio.
#
#   docker build -t dataviz-mcp mcp/
#   docker run -i --rm dataviz-mcp
#
# Self-contained: builds the published crate, so it needs no build context.
FROM rust:1.100-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37 AS build
RUN cargo install dataviz-mcp --locked --root /out

FROM debian:bookworm-slim@sha256:e67eeead736ff7bbe258ef088bff683635c561b1fcfefad0806c1df4ab49ce0e
COPY --from=build /out/bin/dataviz-mcp /usr/local/bin/dataviz-mcp
# Charts saved with an `output` path land here; mount a volume to keep them.
WORKDIR /data
ENTRYPOINT ["dataviz-mcp"]
