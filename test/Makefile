
.PHONY: test up setup down

test:
	make up
	make setup
	sleep 1
	docker-compose exec ansible ssh hunter@sshd -i keys/user/id_rsa echo SUCCESS
	make down

up:
	docker-compose up -d --build

setup:
	make -j 2 setup-sshd setup-ansible
	docker-compose exec ansible ansible-playbook -i hosts localtest.yml
	docker-compose restart sshd

setup-sshd:
	docker cp ./keys/root/id_rsa.pub `docker-compose ps -q sshd`:/root/.ssh/authorized_keys
	docker-compose exec sshd chown root:root /root/.ssh/authorized_keys
	docker-compose exec sshd chmod 600 /root/.ssh/authorized_keys

setup-ansible:
	docker cp ./ansible/hosts `docker-compose ps -q ansible`:/work/
	docker cp ./ansible/localtest.yml `docker-compose ps -q ansible`:/work/
	docker cp ../ansible/roles `docker-compose ps -q ansible`:/work/roles
	docker cp ../ansible/templates `docker-compose ps -q ansible`:/work/templates
	docker cp ../ansible/sectora.sh `docker-compose ps -q ansible`:/work/sectora.sh
	docker cp ./keys `docker-compose ps -q ansible`:/work/keys
	docker cp ../target/ `docker-compose ps -q ansible`:/work/target

down:
	docker-compose down
