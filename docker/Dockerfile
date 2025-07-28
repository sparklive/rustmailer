# Use Debian bullseye-slim as the base image for newer OpenSSL
FROM ubuntu:24.04
ARG CRATE_VERSION

# Set maintainer metadata
LABEL maintainer="rustmailer <rustmailer.git@gmail.com>"
LABEL version="${CRATE_VERSION}"
LABEL description="Dockerized RustMailer service"

# Create application user and group (non-root)
RUN groupadd -r rmailer && useradd -r -g rmailer rmailer

# Set working directory
WORKDIR /opt/rustmailer

# Copy compiled binary (ensure it's statically linked or compatible with bullseye)
COPY --chown=rmailer:rmailer rustmailer /opt/rustmailer/rustmailer
COPY --chown=rmailer:rmailer LICENSE /opt/rustmailer/
COPY --chown=rmailer:rmailer license.html /opt/rustmailer/
# Set proper permissions
RUN chmod +x /opt/rustmailer/rustmailer

# Create data directory
RUN mkdir -p /data && chown rmailer:rmailer /data

# Expose default ports
EXPOSE 15630 16630


# Switch to non-root user
USER rmailer

# Set volume and working directory
VOLUME /data
WORKDIR /data

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD curl -fs http://localhost:15630/api/status || exit 1

# Entrypoint remains the binary
CMD ["/opt/rustmailer/rustmailer"]