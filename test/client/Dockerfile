FROM alpine:3.24.3

RUN apk add openssh ansible sshpass
COPY sshconfig /root/.ssh/config
RUN chmod -R 600 /root/.ssh
