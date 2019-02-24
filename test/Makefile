
.PHONY: test up setup down

test:
	make up
	make setup
	sleep 1
	ssh -p 2222 hunter@localhost -i keys/user/id_rsa echo success
	make down

up:
	docker-compose up -d

setup:
	ansible-playbook -i ansible/hosts ansible/localtest.yml
	docker-compose restart sshd

down:
	docker-compose down
