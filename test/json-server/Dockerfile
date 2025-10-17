FROM node:25-alpine

RUN npm install -g json-server@0.17.4

WORKDIR /data

COPY db.json /data/db.json
COPY routes.json /data/routes.json

EXPOSE 3000
