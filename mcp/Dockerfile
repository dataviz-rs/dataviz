# dataviz-mcp: the dataviz MCP server over stdio.
#
#   docker build -t dataviz-mcp mcp/
#   docker run -i --rm dataviz-mcp
#
# Self-contained: builds the published crate, so it needs no build context.
FROM rust:1.89-slim-bookworm@sha256:d7fc7de78bb8c1469933aeecbf801314d30d7d6e9f0578bba4cfa285bfa37fe6 AS build
RUN cargo install dataviz-mcp --locked --root /out

FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
COPY --from=build /out/bin/dataviz-mcp /usr/local/bin/dataviz-mcp
# Charts saved with an `output` path land here; mount a volume to keep them.
WORKDIR /data
ENTRYPOINT ["dataviz-mcp"]
