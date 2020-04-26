VERSION=$(shell grep "^version" ../Cargo.toml | cut -f 2 -d '"')

.PHONY: test-ansible test-deb up down reset

test-ansible:
	make up
	make setup-ansible
	sleep 1
	make exec-login
	make down

test-deb:
	make up
	make setup-deb
	make exec-login
	make down

setup-deb:
	make -j 2 setup-host-deb setup-client-key

setup-ansible:
	make -j 2 setup-host-root setup-client-ansible
	docker-compose exec client ansible-playbook -i hosts localtest.yml

setup-host-root:
	docker cp ./keys/root/id_rsa.pub `docker-compose ps -q host`:/root/.ssh/authorized_keys
	docker-compose exec host chown root:root /root/.ssh/authorized_keys
	docker-compose exec host chmod 600 /root/.ssh/authorized_keys

setup-client-key:
	docker cp ./keys `docker-compose ps -q client`:/work/keys

setup-host-deb:
	make setup-host-root
	docker cp ../target/x86_64-unknown-linux-gnu/debian/sectora_$(VERSION)_amd64.deb `docker-compose ps -q host`:/root/
	docker-compose exec host sh -c "yes | apt install --yes /root/sectora_$(VERSION)_amd64.deb"
	docker cp ./testconf.toml `docker-compose ps -q host`:/etc/sectora.conf
	docker-compose exec host systemctl start sectora
	docker-compose exec host systemctl restart ssh

setup-client-ansible:
	make setup-client-key
	docker cp ./client/hosts `docker-compose ps -q client`:/work/
	docker cp ./client/localtest.yml `docker-compose ps -q client`:/work/
	docker cp ../ansible/roles `docker-compose ps -q client`:/work/roles
	docker cp ../ansible/templates `docker-compose ps -q client`:/work/templates
	docker cp ../ansible/sectora.sh `docker-compose ps -q client`:/work/sectora.sh
	docker cp ../target/x86_64-unknown-linux-gnu/release `docker-compose ps -q client`:/work/release

up:
	docker-compose up -d --build

exec-login:
	docker-compose exec client ssh hunter@host -i keys/user/id_rsa echo SUCCESS
	docker-compose exec client ssh hunter@host -i keys/user/id_rsa id
	docker-compose exec client ssh hunter@host -i keys/user/id_rsa sectora rate-limit

down:
	docker-compose down

reset:
	cd .. && make x64 LOG_LEVEL=DEBUG
	make down
	make up
	make setup

restart:
	docker-compose restart sshd
