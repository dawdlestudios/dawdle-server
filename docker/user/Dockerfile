FROM debian:bookworm
ARG EXTRA_DEPS="sudo micro git hugo procps build-essential zsh lsd"
ENV DAWDLE_USER="changeme"

# Ensure the package database is up to date
RUN apt-get update -y && \
    apt-get upgrade -y && \
    apt-get install -y ${EXTRA_DEPS} && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* && \
    # install rustup
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    # install bun
    curl -fsSL https://bun.sh/install | bash && \
    # use zsh as default shell
    useradd -u 1000 -m -s /bin/zsh user && \
    usermod -aG sudo user && \
    echo '%sudo ALL=(ALL) NOPASSWD:ALL' >>/etc/sudoers

# Set the default user to be the non-root user
USER 1000

# Ensure some things are set up for the non-root user
COPY entrypoint.sh /entrypoint.sh
RUN sudo chmod +x /entrypoint.sh

CMD ["/bin/zsh", "-c", "/entrypoint.sh"]
