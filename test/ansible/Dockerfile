FROM alpine:3.9

RUN apk add musl-dev python-dev py-pip make gcc openssh libffi-dev openssl-dev
RUN pip install ansible
COPY sshconfig /root/.ssh/config
