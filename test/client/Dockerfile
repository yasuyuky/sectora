FROM alpine:3.24.4

RUN apk add openssh ansible sshpass
COPY sshconfig /root/.ssh/config
RUN chmod -R 600 /root/.ssh
