docker build -t dawdle-home -f docker/user/Dockerfile docker/user
docker run -v ${PWD}/.files/home/henry:/home/henry -e DAWDLE_USER=henry -it dawdle-home
docker exec -it b4c2bae08322 /bin/sh -c "cd ~ && $SHELL"
