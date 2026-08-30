# MCP server image for jscpd (used by Glama for release builds).
# The npm package ships prebuilt Rust binaries per platform.
FROM node:22-slim

RUN npm install -g jscpd@5 && jscpd --version

# The MCP server scans its cwd at startup; give it an empty workdir so it
# doesn't try to scan the container's root filesystem.
WORKDIR /app

# Stdio MCP server: responds to initialize/tools requests on stdin/stdout.
ENTRYPOINT ["jscpd", "--mcp"]
