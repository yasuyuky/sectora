
.PHONY: test up setup down

test:
	make up
	make setup
	sleep 1
	docker-compose run --rm ansible ssh hunter@sshd -i keys/user/id_rsa echo success
	make down

up:
	docker-compose up -d --build

setup:
	docker-compose run --rm ansible ansible-playbook -i hosts localtest.yml
	docker-compose restart sshd

down:
	docker-compose down
