FROM alpine:3.16.0

RUN apk add openssh ansible
COPY sshconfig /root/.ssh/config
RUN chmod -R 600 /root/.ssh
