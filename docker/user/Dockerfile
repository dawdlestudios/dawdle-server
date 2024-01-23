FROM debian:12-slim
ARG EXTRA_DEPS="sudo micro git procps build-essential zsh curl wget vim nano tar gzip unzip bzip2 xz-utils rsync"

USER root

# Ensure the package database is up to date
RUN apt-get update -y && \
    apt-get upgrade -y && \
    apt-get install -y ${EXTRA_DEPS} && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* && \
    # use zsh as default shell
    useradd -u 1000 -m -s /bin/zsh user && \
    usermod -aG sudo user && \
    echo '%sudo ALL=(ALL) NOPASSWD:ALL' >>/etc/sudoers

# Ensure some things are set up for the non-root user
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Set the default user to be the non-root user
USER 1000

CMD ["/bin/zsh", "-c", "/entrypoint.sh"]
