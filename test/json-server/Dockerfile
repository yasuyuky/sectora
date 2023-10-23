FROM node:21-alpine

RUN npm install -g json-server

WORKDIR /data

COPY db.json /data/db.json
COPY routes.json /data/routes.json

EXPOSE 3000
