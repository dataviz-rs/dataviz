# dataviz-mcp: the dataviz MCP server over stdio.
#
#   docker build -t dataviz-mcp mcp/
#   docker run -i --rm dataviz-mcp
#
# Self-contained: builds the published crate, so it needs no build context.
FROM rust:1.89-slim-bookworm AS build
RUN cargo install dataviz-mcp --locked --root /out

FROM debian:bookworm-slim
COPY --from=build /out/bin/dataviz-mcp /usr/local/bin/dataviz-mcp
# Charts saved with an `output` path land here; mount a volume to keep them.
WORKDIR /data
ENTRYPOINT ["dataviz-mcp"]
