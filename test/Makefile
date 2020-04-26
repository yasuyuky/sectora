VERSION=$(shell grep "^version" ../Cargo.toml | cut -f 2 -d '"')

.PHONY: test up setup down reset

test:
	make up
	make setup
	sleep 1
	docker-compose exec ansible ssh hunter@sshd -i keys/user/id_rsa echo SUCCESS
	docker-compose exec ansible ssh hunter@sshd -i keys/user/id_rsa id
	docker-compose exec ansible ssh hunter@sshd -i keys/user/id_rsa sectora rate-limit
	make down

test-deb:
	make up
	make setup-deb
	docker-compose exec ansible ssh hunter@sshd -i keys/user/id_rsa echo SUCCESS
	make down

setup-deb:
	make -j 2 setup-host setup-client-key

setup-host-deb:
	make setup-sshd
	docker cp ../target/x86_64-unknown-linux-gnu/debian/sectora_$(VERSION)_amd64.deb `docker-compose ps -q sshd`:/root/
	docker-compose exec sshd sh -c "yes | apt install --yes /root/sectora_$(VERSION)_amd64.deb"
	docker cp ./testconf.toml `docker-compose ps -q sshd`:/etc/sectora.conf
	docker-compose exec sshd systemctl start sectora
	docker-compose exec sshd systemctl restart ssh

setup-client-key:
	docker cp ./keys `docker-compose ps -q ansible`:/work/keys

up:
	docker-compose up -d --build

setup:
	make -j 2 setup-sshd setup-ansible
	docker-compose exec ansible ansible-playbook -i hosts localtest.yml

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
	docker cp ../target/x86_64-unknown-linux-gnu/release `docker-compose ps -q ansible`:/work/release

down:
	docker-compose down

reset:
	cd .. && make x64 LOG_LEVEL=DEBUG
	make down
	make up
	make setup

restart:
	docker-compose restart sshd
