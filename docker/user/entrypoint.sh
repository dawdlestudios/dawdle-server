#!/bin/bash

# Disabling shell echoing
set +x

# Check if DAWDLE_USER environment variable is set
if [ -n "$DAWDLE_USER" ]; then
    # Use sudo to change the username and home directory
    sudo usermod -l $DAWDLE_USER user
    sudo usermod -d /home/$DAWDLE_USER $DAWDLE_USER
fi

cd ~

# keep the container running
tail -f /dev/null
