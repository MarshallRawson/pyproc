FROM ubuntu:22.04

RUN apt update
RUN apt install -y libpython3-dev sudo curl gcc

WORKDIR /home/root
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

WORKDIR /home/root/pyproc
COPY . .
